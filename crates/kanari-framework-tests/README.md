# Kanari framework tests


## How to build a bitcoin block tester genesis?

1. Download the events file `wget -c https://storage.googleapis.com/kanari_dev/ord_event_blk_858999.tar.zst`
2. Use unzstd and tar to decompressing the file to an event dir.
3. Run

```bash
cargo run -p kanari-framework-tests --  --btc-rpc-url http://localhost:9332 --btc-rpc-username your_username --btc-rpc-password your_pwd --ord-event-dir your_local_event_dir --blocks 790964 --blocks 855396
```

## How to test move contract?

UB=1 cargo test -p kanari-framework-tests $case/$method_name

Fox example:

```bash
UB=1 cargo test -p kanari-framework-tests table/list_field_keys
```
