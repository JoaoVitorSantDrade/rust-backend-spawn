#!/bin/bash

set -x
# Para o script se qualquer comando falhar
set -e

# Defina o nome da sua imagem
DOCKER_IMAGE="jvsandrade/rust-backend-spawn:latest"

# 1. Faz o build da imagem
echo "🚀 Construindo a imagem: $DOCKER_IMAGE"
docker buildx build -t $DOCKER_IMAGE .

set +x
# 2. Faz o push para o Docker Hub
echo "⬆️ Enviando a imagem para o Docker Hub..."
docker push $DOCKER_IMAGE

# 3. Sobe os serviços com Docker Compose
echo "🚀 Iniciando os serviços com Docker Compose..."
docker-compose up -d

echo "✅ Deploy concluído com sucesso!"