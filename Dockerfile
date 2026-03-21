# Build Stage
FROM golang:1.25-alpine AS builder

WORKDIR /app

# Copy all source code first
COPY . .

# Download dependencies and tidy modules
RUN go mod tidy && go mod download

# Build (pure Go, no CGO needed)
RUN CGO_ENABLED=0 GOOS=linux go build -o bot cmd/bot/main.go

# Runtime Stage
FROM alpine:3.18

WORKDIR /app

RUN apk --no-cache add ca-certificates

COPY --from=builder /app/bot .
COPY config.yaml .

CMD ["./bot"]
