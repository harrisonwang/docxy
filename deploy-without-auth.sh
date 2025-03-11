#!/bin/bash
# Simple script to build and run the service without authentication

# Build the service
cargo build --release

# Run the service on port 8080
PORT=8080 ./target/release/docxy
