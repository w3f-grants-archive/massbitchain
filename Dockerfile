################
##### Builder
FROM rust:1.61.0 as builder

WORKDIR /usr/src

# Create blank project
RUN USER=root cargo new massbitchain

# We want dependencies cached, so copy those first.
COPY Cargo.toml Cargo.lock  /usr/src/massbitchain/

# Set the working directory
WORKDIR /usr/src/massbitchain

## Install target platform (Cross-Compilation) --> Needed for Alpine
RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update  && apt install cmake make clang pkg-config libssl-dev -y
# Now copy in the rest of the sources
COPY . /usr/src/massbitchain/

RUN rustup update
RUN rustup update nightly
RUN rustup target add wasm32-unknown-unknown --toolchain nightly

# This is the actual application build.
RUN cargo build  --release

################
# ##### Runtime
FROM debian AS runtime

# Copy application binary from builder image
COPY entrypoint.sh /usr/local/bin
COPY --from=builder /usr/src/massbitchain/target/release/massbit-node /usr/local/bin
RUN apt-get update && apt-get install -y ca-certificates
#EXPOSE 9944

ENTRYPOINT ["bash","entrypoint.sh"]


# ###########################
# FROM debian AS runtime

# # Copy application binary from builder image
# COPY massbit-node /usr/local/bin
# COPY entrypoint.sh .

EXPOSE 9944
# EXPOSE 9933

# # Run the application
# ENTRYPOINT ["bash","entrypoint.sh"]
