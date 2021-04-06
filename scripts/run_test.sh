#!/bin/bash
set -euxo pipefail

target/debug/ecosystem 1 &
target/debug/ecosystem 2 &
