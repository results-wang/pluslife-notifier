FROM ubuntu:24.04

RUN apt-get update && apt-get install -y libfontconfig1-dev curl

COPY target/release/web /pluslife-notifier

CMD ["/pluslife-notifier"]
