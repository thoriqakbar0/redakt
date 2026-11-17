"""Optional grouped-expert CPU inference for the pinned OPF implementation."""

from types import MethodType

import torch
import torch.nn.functional as functional
from opf._model.model import MLPBlock, swiglu


def expert_output(block, tokens, expert_id):
    hidden = functional.linear(
        tokens, block.mlp1_weight[expert_id].T, block.mlp1_bias[expert_id]
    )
    hidden = swiglu(hidden, limit=block.swiglu_limit, packed=block.packed_geglu)
    return functional.linear(
        hidden, block.mlp2_weight[expert_id].T, block.mlp2_bias[expert_id]
    )


def grouped_forward(block, inputs):
    if inputs.dim() != 3:
        raise ValueError('MLPBlock expects batched 3D tensor input')
    normalized = block.norm(inputs).reshape(-1, inputs.shape[-1]).float()
    gates = functional.linear(normalized, block.gate.weight.float(), block.gate.bias.float())
    experts = torch.topk(gates, k=block.experts_per_token, dim=-1, sorted=True)
    weights = functional.softmax(experts.values, dim=1) / block.experts_per_token

    routed_output = normalized.new_empty((*experts.indices.shape, normalized.shape[-1]))
    for expert_id in torch.unique(experts.indices).tolist():
        token_ids, slots = torch.where(experts.indices == expert_id)
        tokens = normalized.index_select(0, token_ids)
        routed_output[token_ids, slots] = expert_output(block, tokens, expert_id)

    combined = torch.einsum('bec,be->bc', routed_output, weights)
    combined = (combined * block.experts_per_token).to(inputs.dtype)
    return inputs + combined.reshape_as(inputs)


def enable_grouped_cpu(model):
    """Group CPU MoE inference and cache expert parameters as float32 in place."""
    blocks = [module for module in model.modules() if isinstance(module, MLPBlock)]
    if not blocks:
        raise ValueError('Model has no supported OPF MLPBlock modules')
    for block in blocks:
        if block.training or block.world_size != 1:
            raise ValueError('Grouped CPU inference requires eval mode and world size one')
        supported = all(
            parameter.device.type == 'cpu'
            and parameter.dtype in (torch.float32, torch.bfloat16)
            for parameter in block.parameters()
        )
        if not supported:
            raise ValueError('Grouped CPU inference requires CPU float32 or bfloat16 parameters')
    for block in blocks:
        for name in ('mlp1_weight', 'mlp1_bias', 'mlp2_weight', 'mlp2_bias'):
            parameter = getattr(block, name)
            cached = torch.nn.Parameter(parameter.detach().float(), requires_grad=False)
            setattr(block, name, cached)
        block.forward = MethodType(grouped_forward, block)
    return model
