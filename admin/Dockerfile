# Dockerfile for Admin UI
FROM nginx:alpine

# Set default API URL (can be overridden at runtime)
ARG API_URL=https://wg0qj9h1b7.execute-api.us-west-2.amazonaws.com/prod
ENV API_URL=${API_URL}

# Copy admin UI files
COPY admin-ui.html /usr/share/nginx/html/index.html

# Copy custom entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/custom-entrypoint.sh
RUN chmod +x /usr/local/bin/custom-entrypoint.sh

# Expose port 80
EXPOSE 80

# Use custom entrypoint to generate config.js before starting nginx
ENTRYPOINT ["/usr/local/bin/custom-entrypoint.sh"]
CMD ["nginx", "-g", "daemon off;"]

