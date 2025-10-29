#!/bin/bash

# Harbor Deployment Script
# This script helps deploy the vault-keeper to Harbor

set -e

# Configuration
REGISTRY="${HARBOR_REGISTRY:-wu1skjk2.c1.de1.container-registry.ovh.net}"
PROJECT="${HARBOR_PROJECT:-keeper}"
IMAGE_NAME="vault-keeper"
TAG="${1:-latest}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 Starting Harbor deployment...${NC}"

# Check if required environment variables are set
if [ -z "$HARBOR_USERNAME" ] || [ -z "$HARBOR_PASSWORD" ]; then
    echo -e "${RED}❌ Error: HARBOR_USERNAME and HARBOR_PASSWORD must be set${NC}"
    echo "Usage: export HARBOR_USERNAME=your-username"
    echo "       export HARBOR_PASSWORD=your-password"
    exit 1
fi

# Build the Docker image
echo -e "${YELLOW}📦 Building Docker image...${NC}"
docker build -t ${REGISTRY}/${PROJECT}/${IMAGE_NAME}:${TAG} .

# Tag for latest if not already latest
if [ "$TAG" != "latest" ]; then
    docker tag ${REGISTRY}/${PROJECT}/${IMAGE_NAME}:${TAG} ${REGISTRY}/${PROJECT}/${IMAGE_NAME}:latest
fi

# Login to Harbor
echo -e "${YELLOW}🔐 Logging in to Harbor...${NC}"
echo "$HARBOR_PASSWORD" | docker login ${REGISTRY} -u "$HARBOR_USERNAME" --password-stdin

# Push the image
echo -e "${YELLOW}📤 Pushing image to Harbor...${NC}"
docker push ${REGISTRY}/${PROJECT}/${IMAGE_NAME}:${TAG}
docker push ${REGISTRY}/${PROJECT}/${IMAGE_NAME}:latest

# Deploy to Harbor (if using Kubernetes)
if [ "$DEPLOY_TO_K8S" = "true" ]; then
    echo -e "${YELLOW}🚀 Deploying to Kubernetes...${NC}"
    
    # Update the image tag in the deployment file
    sed -i.bak "s|your-harbor-registry.com/your-project/vault-keeper:staging|${REGISTRY}/${PROJECT}/${IMAGE_NAME}:${TAG}|g" deploy/harbor-deployment.yml
    
    # Apply the deployment
    kubectl apply -f deploy/harbor-deployment.yml
    
    # Wait for deployment to be ready
    kubectl rollout status deployment/vault-keeper -n vault-keeper --timeout=300s
    
    echo -e "${GREEN}✅ Deployment completed successfully!${NC}"
else
    echo -e "${GREEN}✅ Image pushed to Harbor successfully!${NC}"
    echo -e "${YELLOW}💡 To deploy to Kubernetes, run:${NC}"
    echo "   export DEPLOY_TO_K8S=true"
    echo "   ./scripts/deploy-to-harbor.sh ${TAG}"
fi

# Cleanup
echo -e "${YELLOW}🧹 Cleaning up local images...${NC}"
docker rmi ${REGISTRY}/${PROJECT}/${IMAGE_NAME}:${TAG} || true
docker rmi ${REGISTRY}/${PROJECT}/${IMAGE_NAME}:latest || true

echo -e "${GREEN}🎉 Harbor deployment completed!${NC}"
