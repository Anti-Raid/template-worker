#!/bin/bash
#
# Maintained by Jack Tanner

function pullCommand {
    git pull origin HEAD:master
}

cd $(git rev-parse --show-toplevel)

git submodule init && git submodule update --init --recursive

cd luau
git submodule init && git submodule update --init --recursive

cd builtins
pullCommand

cd templating-types
pullCommand