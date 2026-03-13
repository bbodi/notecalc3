FROM node:lts

RUN npm install -g serve wasm-pack

EXPOSE 5000/tcp
EXPOSE 5000/udp

COPY . .
RUN chmod +x compile_and_run.bat

CMD ./compile_and_run.bat
