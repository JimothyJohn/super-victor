#!/usr/bin/env bash
set -e
set -o nounset
set -o pipefail

[ "${TRACE:-0}" = "1" ] && set -x

STACK_NAME="${STACK_NAME:-supervictor-endpoint-staging}"
HOSTNAME_FQDN="${HOSTNAME_FQDN:-staging.supervictor.advin.io}"
INSTANCE_TYPE="${INSTANCE_TYPE:-t4g.micro}"
HOSTED_ZONE_ID="${HOSTED_ZONE_ID:-Z039212425BG1MHVPYWDN}"
BUCKET="${BUCKET:-supervictor}"
REGION="${AWS_REGION:-us-east-1}"

log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*"; }
command_exists() { command -v "$1" >/dev/null 2>&1; }

for cmd in aws tar openssl; do
    command_exists "$cmd" || { log "missing: $cmd"; exit 1; }
done

REPO_ROOT="$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)"
cd "$REPO_ROOT"

CA_PEM="certs/ca/ca.pem"
CA_KEY="certs/ca/ca.key"
[ -f "$CA_PEM" ] || { log "missing CA cert at $CA_PEM"; exit 1; }
[ -f "$CA_KEY" ] || { log "missing CA key at $CA_KEY"; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

log "issuing ECDSA P-256 server cert for ${HOSTNAME_FQDN} signed by dev CA"
openssl ecparam -genkey -name prime256v1 -out "$WORK/server.key" 2>/dev/null
openssl req -new -key "$WORK/server.key" \
    -subj "/CN=${HOSTNAME_FQDN}/O=Supervictor/OU=Servers" \
    -out "$WORK/server.csr" 2>/dev/null
openssl x509 -req -in "$WORK/server.csr" \
    -CA "$CA_PEM" -CAkey "$CA_KEY" -CAcreateserial \
    -days 365 -sha256 \
    -extfile <(printf "subjectAltName=DNS:%s\nextendedKeyUsage=serverAuth\n" "$HOSTNAME_FQDN") \
    -out "$WORK/server.pem" 2>/dev/null

log "building deploy bundle"
tar --exclude='target' --exclude='.git' --exclude='node_modules' \
    -czf "$WORK/bundle.tar.gz" \
    supervictor/common \
    supervictor/endpoint

log "discovering default VPC + a public subnet in ${REGION}"
VPC_ID="$(aws ec2 describe-vpcs \
    --filters Name=isDefault,Values=true \
    --query 'Vpcs[0].VpcId' --output text --region "$REGION")"
SUBNET_ID="$(aws ec2 describe-subnets \
    --filters Name=vpc-id,Values="$VPC_ID" Name=default-for-az,Values=true \
    --query 'Subnets[0].SubnetId' --output text --region "$REGION")"
log "  vpc=${VPC_ID}  subnet=${SUBNET_ID}"

log "uploading artifacts to s3://${BUCKET}/staging/"
aws s3 cp "$WORK/bundle.tar.gz" "s3://${BUCKET}/staging/bundle.tar.gz"
aws s3 cp "$CA_PEM"             "s3://${BUCKET}/staging/ca.pem"
aws s3 cp "$WORK/server.pem"    "s3://${BUCKET}/staging/server.pem"
aws s3 cp "$WORK/server.key"    "s3://${BUCKET}/staging/server.key"

log "deploying CloudFormation stack ${STACK_NAME}"
aws cloudformation deploy \
    --template-file supervictor/endpoint/template-staging.yaml \
    --stack-name "$STACK_NAME" \
    --capabilities CAPABILITY_IAM \
    --parameter-overrides \
        Hostname="$HOSTNAME_FQDN" \
        InstanceType="$INSTANCE_TYPE" \
        HostedZoneId="$HOSTED_ZONE_ID" \
        BundleBucket="$BUCKET" \
        VpcId="$VPC_ID" \
        SubnetId="$SUBNET_ID" \
    --region "$REGION"

log "stack outputs:"
aws cloudformation describe-stacks \
    --stack-name "$STACK_NAME" \
    --region "$REGION" \
    --query 'Stacks[0].Outputs[*].[OutputKey,OutputValue]' \
    --output table

log "done"
log "note: first boot takes ~5-10 min (docker build of the Rust endpoint)"
log "watch bootstrap:  aws ssm start-session --target <instance-id> --region ${REGION}"
log "                  then: sudo tail -f /var/log/bootstrap.log"
