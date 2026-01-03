#!/bin/bash

# Load environment variables
set -a
source .env
set +a

# Run the server
cargo run --release
