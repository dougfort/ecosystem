# ecosystem

Model ecological system

This is an experiment with the [Tonic](https://github.com/hyperium/tonic) gRPC
implemention.

A set of Organisms connect to each other with bidirectional gRPC streams. Each
Organism produces food on an outgoing stream and consumes food from an incoming
stream.

The goal, which is still in progress, is to evolve a balanced ecology with
organisms producing and consuming in balance.
