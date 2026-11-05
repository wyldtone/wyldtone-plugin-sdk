# Public regression tests

`python3 -m unittest discover -s tests/unit -p 'test_*.py'` runs static
structure checks; `cargo test --release --locked --workspace` tests the Rust
crate functionality. Private third-party plugin fixtures are intentionally
**not included** in the public distribution.

Use `scripts/verify-public.sh` on a fresh Linux build host for representative
end-to-end C++ and Faust builds plus the offline WAP reference host.
