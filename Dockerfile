# Build Stage
FROM golang:1.25-alpine AS builder

WORKDIR /app

# Copy only go.mod first (go.sum will be generated)
COPY go.mod ./

# Generate go.sum and download dependencies
RUN go mod tidy && go mod download

# Copy the rest of the code
COPY . .

# Build (pure Go, no CGO needed)
RUN CGO_ENABLED=0 GOOS=linux go build -o bot cmd/bot/main.go

# Runtime Stage
FROM alpine:3.18

WORKDIR /app

RUN apk --no-cache add ca-certificates

COPY --from=builder /app/bot .
COPY config.yaml .

CMD ["./bot"]
