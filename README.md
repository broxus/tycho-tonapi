## Tycho TONAPI Adapter

A simple lite-server that can be used for block streaming and serving account states.

[**Protobuf API**](./proto/indexer.proto)

## How To Run

Install dependencies:
```bash
sudo apt install build-essential git libssl-dev zlib1g-dev pkg-config clang
```

Build and run the node:
```bash
# Install.
git clone https://github.com/broxus/tycho-tonapi.git
cd tycho-tonapi
cargo install --path . --locked

# Generate and edit the default config.
tycho-tonapi run --init-config config.json

# Download the latest global config (e.g. for tycho testnet).
wget -O global-config.json https://testnet.tychoprotocol.com/global-config.json

# Start the node.
tycho-tonapi run \
  --config config.json \
  --global-config global-config.json \
  --keys keys.json
```

By default the node will listen on the following addresses:
- `0.0.0.0:30000/UDP` for the node itself (`.local_ip` and `.port` fields in the config);
- `127.0.0.1:10000/TCP` for prometheus exporter (`.metrics.listen_addr` field in the config);
- `127.0.0.1:50051/TCP` for the gRPC server (`.grpc.listen_addr` field in the config).

## Contributing

We welcome contributions to the project! If you notice any issues or errors,
feel free to open an issue or submit a pull request.

## License

Licensed under MIT license ([LICENSE](LICENSE) or <https://opensource.org/licenses/MIT>).
