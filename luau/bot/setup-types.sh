#!/bin/bash
cd "$(dirname "$0")"
git pull
cd templating-types
git pull origin HEAD:master
git submodule update --init --recursive
