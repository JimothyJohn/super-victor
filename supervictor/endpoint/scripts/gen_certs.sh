#!/usr/bin/env bash
#
# gen_certs.sh — mTLS certificate issuance for the supervictor fleet.
# Invoked by `qs certs <ca|device|server|admin|list>`; runs from the repo root.
#
# Layout (all under ./certs/, gitignored):
#   ca/ca.key ca/ca.pem                      root CA
#   devices/<name>/client.key client.pem     device identity (OU=devices)
#   servers/<name>/server.key server.pem     server TLS (OU=Servers, SAN)
#   admins/<name>/admin.key admin.pem .p12   dashboard admin (OU=admin)
#
set -e
set -o nounset
set -o pipefail

[ "${TRACE:-0}" = "1" ] && set -x

CERTS_DIR="${CERTS_DIR:-certs}"
ORG="Supervictor"
CA_DAYS="${CA_DAYS:-3650}"
DEFAULT_DAYS=825

log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*"; }
die() { log "ERROR: $*" >&2; exit 1; }

command -v openssl >/dev/null 2>&1 || die "openssl not found"

require_ca() {
    [ -f "$CERTS_DIR/ca/ca.key" ] && [ -f "$CERTS_DIR/ca/ca.pem" ] \
        || die "no CA at $CERTS_DIR/ca — run: qs certs ca"
}

refuse_overwrite() {
    if [ -e "$1" ] && [ "${FORCE:-0}" != "1" ]; then
        die "$1 already exists (set FORCE=1 to overwrite)"
    fi
}

new_key() { # <path>
    (umask 077 && openssl ecparam -genkey -name prime256v1 -out "$1" 2>/dev/null)
}

sign_csr() { # <csr> <out_pem> <days> <extensions>
    openssl x509 -req -in "$1" \
        -CA "$CERTS_DIR/ca/ca.pem" -CAkey "$CERTS_DIR/ca/ca.key" -CAcreateserial \
        -days "$3" -extfile <(printf '%b' "$4") -out "$2" 2>/dev/null
}

gen_ca() {
    refuse_overwrite "$CERTS_DIR/ca/ca.key"
    mkdir -p "$CERTS_DIR/ca"
    new_key "$CERTS_DIR/ca/ca.key"
    openssl req -x509 -new -key "$CERTS_DIR/ca/ca.key" \
        -subj "/CN=$ORG Root CA/O=$ORG/OU=CA" \
        -days "$CA_DAYS" -out "$CERTS_DIR/ca/ca.pem" 2>/dev/null
    log "CA created: $CERTS_DIR/ca/ca.pem (valid $CA_DAYS days)"
    log "NOTE: the CA key never leaves this machine; regenerating it invalidates every issued cert"
}

gen_device() { # <name> [days]
    local name="$1" days="${2:-$DEFAULT_DAYS}" dir
    require_ca
    dir="$CERTS_DIR/devices/$name"
    refuse_overwrite "$dir/client.pem"
    mkdir -p "$dir"
    new_key "$dir/client.key"
    openssl req -new -key "$dir/client.key" \
        -subj "/CN=$name/O=$ORG/OU=devices" -out "$dir/client.csr" 2>/dev/null
    sign_csr "$dir/client.csr" "$dir/client.pem" "$days" "extendedKeyUsage=clientAuth\n"
    rm -f "$dir/client.csr"
    log "device cert: $dir/client.pem (CN=$name, OU=devices, $days days)"
}

gen_server() { # <name> <host_ip> [days]
    local name="$1" host="$2" days="${3:-$DEFAULT_DAYS}" dir san
    require_ca
    dir="$CERTS_DIR/servers/$name"
    refuse_overwrite "$dir/server.pem"
    mkdir -p "$dir"
    case "$host" in
        *[a-zA-Z]*) san="DNS:$host" ;;
        *)          san="IP:$host" ;;
    esac
    new_key "$dir/server.key"
    openssl req -new -key "$dir/server.key" \
        -subj "/CN=$host/O=$ORG/OU=Servers" -out "$dir/server.csr" 2>/dev/null
    sign_csr "$dir/server.csr" "$dir/server.pem" "$days" \
        "subjectAltName=$san\nextendedKeyUsage=serverAuth\n"
    rm -f "$dir/server.csr"
    log "server cert: $dir/server.pem (CN=$host, SAN $san, $days days)"
}

gen_admin() { # <name> [days]
    local name="$1" days="${2:-$DEFAULT_DAYS}" dir pass
    require_ca
    dir="$CERTS_DIR/admins/$name"
    refuse_overwrite "$dir/admin.pem"
    mkdir -p "$dir"
    new_key "$dir/admin.key"
    # OU=admin is what the dashboard's auth gate matches on (endpoint src/ui).
    openssl req -new -key "$dir/admin.key" \
        -subj "/CN=$name/O=$ORG/OU=admin" -out "$dir/admin.csr" 2>/dev/null
    sign_csr "$dir/admin.csr" "$dir/admin.pem" "$days" "extendedKeyUsage=clientAuth\n"
    rm -f "$dir/admin.csr"

    # PKCS#12 bundle for browser import. Password via env (never argv, never
    # in the bundle dir); printed once unless the caller supplied their own.
    pass="${P12_PASSWORD:-$(openssl rand -hex 8)}"
    P12PASS="$pass" openssl pkcs12 -export \
        -inkey "$dir/admin.key" -in "$dir/admin.pem" \
        -certfile "$CERTS_DIR/ca/ca.pem" -name "$ORG admin: $name" \
        -passout env:P12PASS -out "$dir/admin.p12"
    chmod 600 "$dir/admin.p12"

    log "admin cert: $dir/admin.pem (CN=$name, OU=admin, $days days)"
    log "browser bundle: $dir/admin.p12"
    if [ -z "${P12_PASSWORD:-}" ]; then
        log "P12 import password (shown once): $pass"
    fi
}


verify_chain() { # [device_name] [server_name]
    local device="${1:-esp32}" server="${2:-caddy}" rc=0
    local ca="$CERTS_DIR/ca/ca.pem"
    [ -f "$ca" ] || die "no CA at $ca"
    log "verify root CA"
    openssl verify -CAfile "$ca" "$ca" || rc=1
    log "verify server cert ($server) against CA"
    openssl verify -CAfile "$ca" "$CERTS_DIR/servers/$server/server.pem" || rc=1
    log "verify device cert ($device) against CA"
    openssl verify -CAfile "$ca" "$CERTS_DIR/devices/$device/client.pem" || rc=1
    log "server cert SAN:"
    openssl x509 -in "$CERTS_DIR/servers/$server/server.pem" -noout -ext subjectAltName || rc=1
    [ "$rc" -eq 0 ] && log "chain OK" || die "some checks failed"
}

handshake() { # <host> <port> [device_name] [tls_version e.g. tls1_3]
    local host="${1:?usage: handshake <host> <port> [device] [tls1_2|tls1_3]}"
    local port="${2:?usage: handshake <host> <port> [device] [tls1_2|tls1_3]}"
    local device="${3:-esp32}" tlsver="${4:-tls1_3}"
    local ca="$CERTS_DIR/ca/ca.pem"
    local dir="$CERTS_DIR/devices/$device"
    log "mTLS handshake to $host:$port as $device ($tlsver)"
    local out
    out="$(openssl s_client -connect "$host:$port" \
        -cert "$dir/client.pem" -key "$dir/client.key" -CAfile "$ca" \
        "-$tlsver" </dev/null 2>&1)" || true
    if printf '%s' "$out" | grep -q "Verify return code: 0 (ok)"; then
        log "handshake OK — Verify return code: 0 (ok)"
    else
        printf '%s\n' "$out" | grep "Verify return code:" || printf '%s\n' "$out" | tail -3
        die "handshake failed"
    fi
}

list_certs() {
    [ -d "$CERTS_DIR" ] || die "no $CERTS_DIR directory"
    find "$CERTS_DIR" -name '*.pem' ! -name 'AmazonRootCA1.pem' | sort | while read -r pem; do
        subject="$(openssl x509 -in "$pem" -noout -subject 2>/dev/null | sed 's/^subject=//')" || continue
        expires="$(openssl x509 -in "$pem" -noout -enddate 2>/dev/null | sed 's/^notAfter=//')"
        printf '%-52s %s (expires %s)\n' "$pem" "$subject" "$expires"
    done
}

usage() {
    cat <<EOF
Usage: $(basename "$0") <mode> [args]

Modes:
  ca                          Initialize the root CA (once)
  device <name> [days]        Issue a device client cert   (OU=devices)
  server <name> <host> [days] Issue a server TLS cert      (OU=Servers, SAN)
  admin  <name> [days]        Issue a dashboard admin cert (OU=admin) + .p12
  list                        List issued certs with subjects and expiry
  verify [device] [server]    Verify the cert chain against the CA
  handshake <host> <port> [device] [tlsver]  Live mTLS handshake test

Env: CERTS_DIR (default: certs), FORCE=1 to overwrite, CA_DAYS,
     P12_PASSWORD (admin mode; random + printed once if unset)
EOF
}

case "${1:-}" in
ca)      gen_ca ;;
device)  [ $# -ge 2 ] || die "usage: gen_certs.sh device <name> [days]"; gen_device "$2" "${3:-}" ;;
server)  [ $# -ge 3 ] || die "usage: gen_certs.sh server <name> <host> [days]"; gen_server "$2" "$3" "${4:-}" ;;
admin)   [ $# -ge 2 ] || die "usage: gen_certs.sh admin <name> [days]"; gen_admin "$2" "${3:-}" ;;
list)    list_certs ;;
verify)  verify_chain "${2:-}" "${3:-}" ;;
handshake) shift; handshake "$@" ;;
-h|--help|help|"") usage ;;
*) usage; die "unknown mode: $1" ;;
esac
