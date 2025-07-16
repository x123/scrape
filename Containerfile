# Stage 1: Build the Rust application
FROM rust:1.88.0-alpine3.22 AS builder

# Set the working directory inside the container
WORKDIR /app

RUN apk add --no-cache pkgconfig libressl-dev musl-dev

# Copy Cargo.toml and Cargo.lock first to leverage Docker cache
# This ensures that if only source code changes, dependencies aren't re-downloaded
COPY Cargo.toml Cargo.lock ./

# Copy the source code
COPY src ./src

RUN cargo build --release

# Stage 2: Create the final, minimal image
# Use a minimal base image, like Debian slim or Alpine, for the final executable
FROM alpine:3.22

# Set the working directory
WORKDIR /usr/local/bin

# Copy the compiled executable from the builder stage
COPY --from=builder /app/target/release/scrape .

# Expose the port that the Actix-Web server listens on
EXPOSE 8282

# Set the entrypoint to run the application
CMD ["./scrape"]
