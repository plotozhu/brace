# “Pnyx” Blockchain
A shard-enabled blockchain achieving 64K TPS (using 3Mbps) on public network having 30Mbps upstream, or 1M TPS on public network having 50M upstream.

## performance 

||	One shard	|1024 shards	|16384 shards|
|-----|----------|---------------|-------------|
|TPS|	62.5|	64,000	|1,000,000|
|Block Generate Time	|15s	|15s	|15s|
|Average Finalization Time |	1 Block|	4 Blocks	|4 Blocks|
|Average Block Size	|256K(for transaction)	|256K ( for transaction ) + 32K ( for cross shard communication )	|256K (for transaction ) + 512K (for cross shard communication )|
|Average Bandwidth Consumption|	1.5Mbps	|3Mbps	|9Mbps|
|Minimum nodes requirement	|1K	|1M	|16M|


## Contributions & Code of Conduct

Please follow the contributions guidelines as outlined in [`docs/CONTRIBUTING.adoc`](docs/CONTRIBUTING.adoc). In all communications and contributions, this project follows the [Contributor Covenant Code of Conduct](docs/CODE_OF_CONDUCT.adoc).

## Security

The security policy and procedures can be found in [`docs/SECURITY.md`](docs/SECURITY.md).

## License

Pnyxchain's source code is [GPL 3.0 licensed](LICENSE), and the algorithms are patented, please contact [here](mailto://license@pnyxchain.fund) for license. 

## appreciation
This implementation is built on top of substrate from ParityTech, thanks for their hard and talent work!
