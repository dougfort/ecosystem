#!/bin/bash
set -euxo pipefail

target/debug/ecosystem --index=1 -n org001 -i bbb -o aaa &
target/debug/ecosystem --index=2 -n org002 -i aaa -o bbb &
