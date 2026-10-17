# Betriebs-Image suchlage:m1 (mise run image). Kontext ist das Repo, nicht deploy/.

# Stufe 1: Oberfläche
FROM node:26.7.0-bookworm-slim AS web
# Node 25+ liefert corepack nicht mehr mit; pnpm kommt deshalb per npm in der gepinnten Version.
RUN npm install -g pnpm@11.10.0
WORKDIR /src/frontend
COPY frontend/package.json frontend/pnpm-lock.yaml frontend/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY frontend/ ./
RUN pnpm build

# Stufe 2: Server (ohne Feature dev – kein Login ohne OIDC im Image)
FROM rust:1.99.0-bookworm AS server
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs rust-toolchain.toml ./
COPY .cargo .cargo
COPY migrations migrations
COPY src src
COPY --from=web /src/frontend/dist frontend/dist
# Setzt der Release-Workflow; leer (lokal) meldet der Server `<Cargo-Version>-dev` (src/version.rs).
ARG SUCHLAGE_VERSION=""
ARG SUCHLAGE_COMMIT=""
RUN cargo build --release --bin suchlage

# /data mit den Rechten des nonroot-Nutzers (distroless kann weder mkdir noch chown); Docker
# übernimmt sie beim ersten Mount eines frischen Volumes (Muster aus Lifeline).
FROM busybox:1.37.0-uclibc AS datadir
RUN mkdir -p /data

# Stufe 3: Laufzeit
FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=datadir --chown=nonroot:nonroot /data /data
COPY --from=server /src/target/release/suchlage /usr/local/bin/suchlage
ENV SUCHLAGE_DATA_DIR=/data SUCHLAGE_BIND=0.0.0.0:8080
VOLUME ["/data"]
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/suchlage"]
