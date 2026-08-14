#!/usr/bin/env bash

# CoolerControl Plugin Installer
# This is a placeholder to help automate plugin installation without requiring 
# compilation and build dependency installation on the user's machine.
#
# Essentially the user can use curl to execute this bash script, which will download
# release assets created by GitHub Actions/GitLab CI, and install them in CC's plugins directory.
#
# Example:
#   curl -fsSL https://example.com/install.sh | bash -s -- \
#     --plugin-name cc-sample-plugin \
#     --url https://example.com/releases/cc-sample-plugin-linux-amd64
