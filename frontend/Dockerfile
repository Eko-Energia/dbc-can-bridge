FROM node:16-alpine

WORKDIR /app

# Copy package.json and package-lock.json
COPY package*.json ./

# Install dependencies
RUN npm install

# Create directories and empty style file if it doesn't exist
RUN mkdir -p public src
RUN touch src/style.css

# Copy application code
COPY public/ ./public/
COPY src/ ./src/

# Expose the port
EXPOSE 3000

# Start the application
CMD ["npm", "start"]