#!/usr/bin/env bash

# Prompt for password on Macos
osascript -e 'text returned of (display dialog "[sudo] Enter password:" default answer "" with hidden answer)' 2> /dev/null
