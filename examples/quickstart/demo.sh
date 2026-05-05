#!/usr/bin/env bash
# Tiny consumer that prints the env vars klef injected.
# Run via:  klef run -- ./demo.sh

echo "── env reçu par le process enfant ──"
echo "STRIPE_KEY    = ${STRIPE_KEY:-(vide)}"
echo "PORT          = ${PORT:-(vide)}"
echo "DATABASE_URL  = ${DATABASE_URL:-(vide)}"
