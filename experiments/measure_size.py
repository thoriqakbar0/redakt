"""Record logical deployment bytes with explicit runtime inclusion boundaries."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import sysconfig


def tree_bytes(root, excluded=()):
    total = 0
    for path in root.rglob('*'):
        if path.is_symlink() or not path.is_file():
            continue
        if any(part in excluded for part in path.relative_to(root).parts):
            continue
        total += path.stat().st_size
    return total


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-source", type=Path, required=True)
    args = parser.parse_args()
    upstream = args.baseline_source / "opf"
    if not upstream.is_dir():
        parser.error("baseline source must contain the opf package")
    executable = Path(sys.executable).resolve()
    stdlib = Path(sysconfig.get_path('stdlib'))
    shared_name = sysconfig.get_config_var('LDLIBRARY')
    shared_path = Path(sysconfig.get_config_var('LIBDIR')) / shared_name
    runtime = {
        'executable_bytes': executable.stat().st_size,
        'stdlib_bytes_excluding_site_packages_and_bytecode': tree_bytes(
            stdlib, ('site-packages', '__pycache__')),
        'libpython_bytes': shared_path.stat().st_size if shared_path.exists() else 0,
    }
    source_bytes = Path('detector.py').stat().st_size
    runtime_bytes = sum(runtime.values())
    baseline_packages = tree_bytes(Path('.venv-baseline/lib'))
    checkpoint = Path('checkpoints/privacy-filter/original')
    checkpoint_bytes = tree_bytes(checkpoint)
    result = {
        'unit': 'logical bytes, uncompressed, symlinks excluded except resolved executable',
        'rules_source_bytes': source_bytes,
        'rules_weight_bytes': 0,
        'python_runtime_components': runtime,
        'rules_plus_python_runtime_bytes': source_bytes + runtime_bytes,
        'baseline_checkpoint_bytes': checkpoint_bytes,
        'baseline_environment_lib_bytes': baseline_packages,
        'baseline_upstream_source_bytes': tree_bytes(upstream, ('__pycache__',)),
        'baseline_plus_python_runtime_bytes': checkpoint_bytes + baseline_packages + runtime_bytes
        + tree_bytes(upstream, ('__pycache__',)),
        'exclusions': 'OS, libc and other system libraries, caches outside env, uv, git, corpus, '
        'evaluation scripts, downloads. Includes installed baseline package bytecode. '
        'Includes the installed libpython even if this interpreter is statically linked. '
        'This is a deployment-footprint estimate, not a standalone bundled executable.',
        'python': sys.version,
        'linked_system_libraries_not_counted': subprocess.check_output(
            ['ldd', str(executable)], text=True),
    }
    result['grouped_adapter_source_bytes'] = Path('experiments/optimized_opf.py').stat().st_size
    result['grouped_baseline_plus_python_runtime_bytes'] = (
        result['baseline_plus_python_runtime_bytes'] + result['grouped_adapter_source_bytes']
    )
    Path('results/size.json').write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
