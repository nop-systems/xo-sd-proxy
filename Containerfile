FROM rust as builder
WORKDIR /usr/src/xo-sd-proxy
COPY . .
RUN cargo install --path .

FROM fedora:latest
COPY --from=builder /usr/local/cargo/bin/xo-sd-proxy /usr/local/bin/xo-sd-proxy
COPY templates /templates
ENV ROCKET_TEMPLATE_DIR=/templates
EXPOSE 8000
CMD ["xo-sd-proxy"]
