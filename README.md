# AIrtifex

Self-hosted, generative AI server and a web app. The API provides the necessary endpoints for interacting with the generative models, while the web app serves as a client-side rendered WASM application for user interaction. The entire project is written in Rust.



![Preview GIF](https://raw.githubusercontent.com/vv9k/airtifex/master/assets/preview.gif)


## Table of Contents

- [Prerequisites](#prerequisites)
- [Setup](#setup)
- [Getting the weights](#getting-the-weights)
- [API Configuration](#api-configuration)
- [Building and Running the Project](#building-and-running-the-project)
  - [Running with Docker](#running-with-docker)
  - [API With SQLite](#api-with-sqlite)
  - [API With PostgreSQL](#api-with-postgresql)
  - [Web App](#web-app)
  - [Systemd service](#systemd-service)
  - [Nginx reverse proxy](#nginx-reverse-proxy)

## Prerequisites

To work with this project, you will need the following tools installed:

- [Rust](https://www.rust-lang.org/tools/install): nightly compiler
- [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html): latest version
- [Trunk](https://trunkrs.dev/#install)
- [Make](https://www.gnu.org/software/make/)
- [LibTorch](https://github.com/LaurentMazare/tch-rs#getting-started)

## Setup

* Clone the repository:

```sh
git clone https://github.com/vv9k/airtifex.git
cd airtifex
```

## Getting the weights

This repository doesn't contain any models/weights, you'll need to get them yourself before running the server. Currently, only LLaMa based models are supported like
Alpaca, Vicuna etc.

For image generation Stable Diffusion models can be used. Below are links to download pretrained weights:
 * https://huggingface.co/lmz/rust-stable-diffusion-v2-1
 * https://huggingface.co/lmz/rust-stable-diffusion-v1-5

After the models are downloaded, we can specify their location in the configuration.

## API Configuration

Below is an example configuration for the server that loads a single 7B Alpaca model for text generation as well as Stable Diffusion v2.1 and v1.5:

```yaml
---
listen_addr: 127.0.0.1
listen_port: 6901
db_url: sqlite://data.db
#db_url: postgres://airtifex:airtifex@localhost/airtifex
jwt_secret: change-me!

llms:
  - model_path: ./llm_models/ggml-alpaca-7b-q4.bin
    model_description: Alpaca 7B, quantized
    float16: false

stable_diffusion:
  - version: v2.1
    name: sd-v2.1
    model_description: Stable Diffusion v2.1
    clip_weights_path: ./sd_models/clip_v2.1.ot
    vae_weights_path: ./sd_models/vae_v2.1.ot
    unet_weights_path: ./sd_models/unet_v2.1.ot
    vocab_file: ./sd_models/bpe_simple_vocab_16e6.txt
```

## Building and Running the Project

Default username and password to API are both `admin`.

### Running with Docker

The simplest way to run this project is to run it using docker and docker-compose. To do so, run:
```sh
make run_docker
```

This will build the image and run the api and web app in a container behind nginx reverse proxy. The [docker-compose.yaml](https://github.com/vv9k/airtifex/blob/master/docker-compose.yaml) file contains example on how to run the app. It mounts the [data](https://github.com/vv9k/airtifex/blob/master/data) directory as a volume which contains the databse file as well as text/image models (you'll have to put the models there or change the source location of the volume before running).

The app will be accessible at http://localhost:8091

The API can also be accessed through the same port like http://localhost:8091/api/v1/llm/inference

### API with Sqlite

To build and run the project using SQLite as the database, follow these steps:

* To run directly use:

```sh
# start the server
cd airtifex-api
make serve_release
```
* To build use:
```sh
cd airtifex-api
make build_release
```
The binary will be in the `target/release` directory after the build succeeds.


### API with PostgreSQL

To build and run the project using PostgreSQL as the database, follow these steps:

* Set up a PostgreSQL database and update the db_url field in the API configuration file (e.g., `airtifex-api/config.yaml`).

* Run directly:
```sh
cd airtifex-api
make serve_release_pg
```

* Build the API server with PostgreSQL support:
```sh
cd airtifex-api
make build_release_pg
```

### Web App

In another terminal start the web app:
```sh
cd airtifex-web
make serve_release
```

The web app will be accessible at http://localhost:8080 by default and is configured to connect to the API server at localhost:6901. To configure it change the values in the `Trunk.toml` file.


### Systemd Service

Example systemd service for the api server can be found [here](https://github.com/vv9k/airtifex/blob/master/assets/airtifex-api.service)


### Nginx reverse proxy

Example configuration to run behind nginx reverse proxy can be found [here](https://github.com/vv9k/airtifex/blob/master/assets/nginx-vhost.conf)


## License
[GPLv3](https://github.com/vv9k/airtifex/blob/master/COPYING)
