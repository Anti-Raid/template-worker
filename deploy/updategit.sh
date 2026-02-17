#!/bin/bash
#
# Maintained by Jack Tanner

function pullCommand {
    git pull origin HEAD:master
}

git submodule init && git submodule update --init --recursive

cd ../luau/builtins
pullCommand

cd templating-types
pullCommand