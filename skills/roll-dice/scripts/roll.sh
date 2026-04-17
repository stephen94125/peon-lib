#!/usr/bin/env bash
# Roll a die with N sides. Default: 6.
# Usage: ./roll.sh [sides]
sides="${1:-6}"
echo $(( (RANDOM % sides) + 1 ))
