# powderman as a container, for Fly (production and PR previews).
#
# A preview has no herdr and no systemd — the daemon reaches for both and treats
# their absence as "unreachable", so the workbench UI, the command bus, the
# palette, the widgets and the MCP server all work; only the fleet and run
# execution go dark. That is exactly what a preview is for: the interface.
#
# Built with cargo-chef so the dependency compile is its OWN layer, keyed on the
# lockfile, not the source. Without it every build recompiles all of dioxus +
# rusqlite from scratch (~9 min), and seven concurrent PR deploys time out Fly's
# shared remote builder. With it, the deps layer is cooked once and cached; a
# push that only changes application code reuses it and finishes in a minute.

FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked --version ^0.1
WORKDIR /src

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
# rusqlite's `bundled` feature compiles SQLite from C.
RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential pkg-config \
 && rm -rf /var/lib/apt/lists/*
# Cook only the dependencies first — this layer caches until the lockfile moves.
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
# Then the application, which is all that recompiles on a normal push.
COPY . .
RUN cargo build --release -p powderman

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/powderman /usr/local/bin/powderman

# Fly routes to internal_port 8080; the DB is on the ephemeral container fs (a
# preview keeps no state); schedules are OFF (never fire the 06:00 sweep); the
# MCP host guard is open because Fly reaches the app under its *.fly.dev name.
ENV POWDERMAN_PORT=8080 \
    POWDERMAN_DB=/data/powderman.db \
    POWDERMAN_SCHEDULES=0 \
    POWDERMAN_MCP_ALLOWED_HOSTS=*
RUN mkdir -p /data
EXPOSE 8080
CMD ["powderman"]
