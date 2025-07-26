#!/bin/bash

set -x
# Para o script se qualquer comando falhar
set -e

# Defina o nome da sua imagem
BASE_IMAGE="jvsandrade/rust-backend-spawn"

# Define o tag padrão como 'latest'
TAG="latest"

# Verifica se o primeiro argumento ($1) é "dev"
if [ "$1" == "dev" ]; then
    TAG="dev"
    echo " MODO DE DESENVOLVIMENTO: A imagem será construída localmente, mas não será enviada para o Docker Hub nem usada pelo docker-compose."
fi

DOCKER_IMAGE="$BASE_IMAGE:$TAG"

# 1. Faz o build da imagem
echo "🚀 Construindo a imagem: $DOCKER_IMAGE"
docker buildx build -t $DOCKER_IMAGE .

set +x
# 2. Faz o push para o Docker Hub
echo "⬆️ Enviando a imagem para o Docker Hub..."
docker push $DOCKER_IMAGE

# 3. Sobe os serviços com Docker Compose
echo "🚀 Iniciando os serviços com Docker Compose..."
DOCKER_TAG=$TAG docker-compose up -d
echo "✅ [Deploy $TAG] concluído com sucesso!"
