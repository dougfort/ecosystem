#!/bin/bash
set -euxo pipefail

target/debug/ecosystem org1 [::1]:10001 [::1]:10002 [::1]:10003 [::1]:10004 [::1]:10005 &
target/debug/ecosystem org2 [::1]:10002 [::1]:10003 [::1]:10004 [::1]:10005 [::1]:10001 &
target/debug/ecosystem org3 [::1]:10003 [::1]:10004 [::1]:10005 [::1]:10001 [::1]:10002 &
target/debug/ecosystem org4 [::1]:10004 [::1]:10005 [::1]:10001 [::1]:10002 [::1]:10003 &
target/debug/ecosystem org5 [::1]:10005 [::1]:10001 [::1]:10002 [::1]:10003 [::1]:10004 &
