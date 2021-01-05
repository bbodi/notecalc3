FROM rnsloan/wasm-pack

RUN apt update && apt install -y nodejs npm
RUN npm install -g serve

EXPOSE 5000/tcp
EXPOSE 5000/udp

RUN git clone https://github.com/bbodi/notecalc3.git .
RUN chmod +x compile_and_run.bat

CMD ./compile_and_run.bat
