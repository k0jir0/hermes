FROM oven/bun:1.2.15
WORKDIR /app

COPY gateway/package.json gateway/tsconfig.json ./
RUN bun install

COPY gateway/src ./src
COPY gateway/test ./test

CMD ["bun", "run", "start"]
