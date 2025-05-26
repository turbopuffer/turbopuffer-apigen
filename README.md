# turbopuffer API generator

This repository contains a supplemental code generator for turbopuffer's API
clients.

If you're a turbopuffer customer, you're probably looking for our API clients
themselves:

  * [turbopuffer-go](https://github.com/turbopuffer/turbopuffer-go)
  * [turbopuffer-python](https://github.com/turbopuffer/turbopuffer-python)
  * [turbopuffer-typescript](https://github.com/turbopuffer/turbopuffer-typescript)
  * [turbopuffer-java](https://github.com/turbopuffer/turbopuffer-java)

## Developer instructions

If you're a turbopuffer developer, here's a bit more context.

[Stainless](https://stainless.com) generates the bulk of the code in our API
clients. However Stainless understandably can't handle our "turbolisp" syntax,
notably used in the `filter` and `rank_by` query parameters. This repository
fills the gap.

The code generator is written in Rust. It is packaged into a Docker image that
is automatically built and pushed to GitHub Container Registry on every merge to
`main`.

The code generator is intended to be invoked in the context of a Stainless
API client repository. It reads the Stainless `stats.yml` file to determine
the OpenAPI specification in use, downlaods that open OpenAPI specification,
and then prints the generated code for the `filter` and `rank_by` types to
stdout. It's up to the CI scripts in each API client repository to wire up
the generator appropriately.
