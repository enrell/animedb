FROM golang:1.25.3-alpine3.22

WORKDIR /app

COPY go.mod go.sum ./
RUN go mod download

COPY . .

EXPOSE 8081

CMD ["go", "run", "./cmd/api", "--listen", ":8081"]
