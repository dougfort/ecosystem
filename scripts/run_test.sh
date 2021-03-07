#!/bin/bash
set -euxo pipefail

target/debug/ecosystem 1 2 &
target/debug/ecosystem 2 2 &
