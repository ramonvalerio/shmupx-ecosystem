# Emuladores do Firebase (Firestore + Authentication) para o ambiente de desenvolvimento.
# Produção é o projeto real (shmupx-project-id, banco "(default)"); este container nunca fala com ele:
# o projeto do emulador é "demo-shmupx", e projetos "demo-*" não alcançam recursos reais.
FROM node:22-bookworm-slim

# Os emuladores do Firestore e da UI rodam na JVM, e as versões atuais pedem Java 21 ou mais novo
# (o Debian desta imagem só tem o 17): JRE oficial da Temurin.
COPY --from=eclipse-temurin:21-jre /opt/java/openjdk /opt/java/openjdk
ENV JAVA_HOME=/opt/java/openjdk
ENV PATH="${JAVA_HOME}/bin:${PATH}"

RUN npm install -g firebase-tools@15.34.1 \
    && firebase setup:emulators:firestore \
    && firebase setup:emulators:ui

WORKDIR /firebase
# firebase.docker.json, firestore.rules e firestore.indexes.json chegam por volume (docker-compose),
# para uma mudança de regra valer sem reconstruir a imagem.
EXPOSE 4000 8088 9099

# --import/--export-on-exit: os dados de desenvolvimento sobrevivem a reinícios do container.
CMD ["firebase", "emulators:start", "--project", "demo-shmupx", "--config", "firebase.docker.json", \
     "--only", "firestore,auth,ui", "--import", "/data", "--export-on-exit", "/data"]
