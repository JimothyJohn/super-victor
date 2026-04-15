#!/usr/bin/env bash
set -e
set -o nounset
set -o pipefail

STACK_NAME="${STACK_NAME:-supervictor-endpoint-staging}"
REGION="${AWS_REGION:-us-east-1}"

log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*"; }

log "deleting stack ${STACK_NAME} in ${REGION}"
aws cloudformation delete-stack --stack-name "$STACK_NAME" --region "$REGION"

log "waiting for delete to complete"
aws cloudformation wait stack-delete-complete --stack-name "$STACK_NAME" --region "$REGION"

log "done"
