# syntax=docker/dockerfile:1

FROM rust:1.88-bookworm

RUN apt update
RUN apt install -y clang lld

# Set destination for COPY

# Copy the source code. Note the slash at the end, as explained in
# https://docs.docker.com/engine/reference/builder/#copy
COPY .cargo/ ./.cargo/
RUN ls ./.cargo/config.toml

# Build the rust project
RUN  --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/app/target \
    SQLX_OFFLINE=true cargo build --release && \
    # Copy executable out of the cache so it is available in the final image.
    cp target/release/template-worker /app/template-worker

WORKDIR /app

# To bind to a TCP port, runtime parameters must be supplied to the docker command.
# But we can (optionally) document in the Dockerfile what ports
# the application is going to listen on by default.
# https://docs.docker.com/engine/reference/builder/#expose
EXPOSE 60000

# Run
CMD [ "/app/template-worker" ]
