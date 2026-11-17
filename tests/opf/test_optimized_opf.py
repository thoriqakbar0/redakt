import copy
import itertools
import unittest

import torch
from opf._model.model import MLPBlock, ModelConfig

from experiments.optimized_opf import enable_grouped_cpu


@torch.no_grad()
def random_block(packed, dtype):
    config = ModelConfig(
        hidden_size=16,
        intermediate_size=24,
        num_experts=7,
        experts_per_token=4,
        param_dtype=dtype,
        packed_geglu=packed,
        torch_ops_batch=5,
    )
    block = MLPBlock(config).eval()
    for parameter in block.parameters():
        parameter.uniform_(-0.3, 0.3)
    return block


class GroupedMoeTests(unittest.TestCase):
    @torch.inference_mode()
    def test_routing_matches_original(self):
        torch.manual_seed(420)
        torch.set_num_threads(1)
        cases = itertools.product([False, True], ['float32', 'bfloat16'], [(1, 1, 16), (2, 13, 16)])
        for packed, dtype, shape in cases:
            reference = random_block(packed, dtype)
            candidate = enable_grouped_cpu(copy.deepcopy(reference))
            inputs = torch.randn(shape).to(reference.gate.weight.dtype)
            expected = reference(inputs)
            actual = candidate(inputs)
            torch.testing.assert_close(actual, expected, rtol=1e-5, atol=1e-6)

    def test_cpu_contract(self):
        block = random_block(False, 'float32')
        block.train()
        with self.assertRaises(ValueError):
            enable_grouped_cpu(block)
        block.eval().half()
        with self.assertRaises(ValueError):
            enable_grouped_cpu(block)


if __name__ == '__main__':
    unittest.main()
