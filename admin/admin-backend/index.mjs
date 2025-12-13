import { S3Client, GetObjectCommand, PutObjectCommand } from '@aws-sdk/client-s3';
import { DynamoDBClient } from '@aws-sdk/client-dynamodb';
import { DynamoDBDocumentClient, GetCommand, PutCommand, DeleteCommand } from '@aws-sdk/lib-dynamodb';
import { parse as parseToml, stringify as stringifyToml } from '@iarna/toml';
import crypto from 'crypto';
// Note: Install ethers with: npm install ethers@5.7.2
import { ethers } from 'ethers';

// Configuration from Environment Variables
const S3_REGION = process.env.S3_REGION;
const BUCKET = process.env.S3_BUCKET;
const KEY = process.env.S3_KEY;
const DYNAMODB_TABLE = process.env.DYNAMODB_TABLE || 'vault-keeper-auth'; // DynamoDB table for tokens and nonces
const AWS_REGION = process.env.AWS_REGION || S3_REGION || 'eu-central-1';

// Initialize AWS clients
const s3Client = new S3Client({ region: S3_REGION });
const dynamoClient = DynamoDBDocumentClient.from(new DynamoDBClient({ 
    region: AWS_REGION 
}));

// Load allowlist from environment variable
// Format: comma-separated addresses (e.g., "0x123...,0x456...")
function getAllowlist() {
    const allowlistStr = process.env.ALLOWLIST || '';
    
    if (!allowlistStr) {
        console.warn('ALLOWLIST environment variable not set, denying all access');
        return [];
    }
    
    // Parse allowlist (comma-separated addresses)
    const addresses = allowlistStr
        .split(',')
        .map(addr => addr.trim())
        .filter(addr => addr.length > 0)
        .map(addr => addr.toLowerCase());
    
    return addresses;
}

// Cache allowlist (loaded once per Lambda instance)
const ALLOWLIST = getAllowlist();

// Generate a secure token using crypto.randomBytes
async function generateToken(address) {
    const timestamp = Date.now();
    // Use crypto.randomBytes(32) for cryptographically secure random data
    const randomBytes = crypto.randomBytes(32);
    const random = randomBytes.toString('base64');
    const token = Buffer.from(`${address}:${timestamp}:${random}`).toString('base64');
    
    // Store token in DynamoDB
    try {
        await dynamoClient.send(new PutCommand({
            TableName: DYNAMODB_TABLE,
            Item: {
                pk: `TOKEN#${token}`,
                sk: 'TOKEN',
                address: address.toLowerCase(),
                timestamp: timestamp,
                ttl: Math.floor(timestamp / 1000) + (24 * 60 * 60) // 24 hours TTL
            }
        }));
    } catch (e) {
        console.error('Failed to store token in DynamoDB:', e);
        throw new Error(`DynamoDB error storing token: ${e.message}. Check Lambda IAM permissions and table: ${DYNAMODB_TABLE}`);
    }
    
    return token;
}

// Verify token from DynamoDB
async function verifyToken(token) {
    try {
        // Check if token exists in DynamoDB
        const result = await dynamoClient.send(new GetCommand({
            TableName: DYNAMODB_TABLE,
            Key: {
                pk: `TOKEN#${token}`,
                sk: 'TOKEN'
            }
        }));

        if (!result.Item) {
            console.warn('Token not found in DynamoDB - possible forgery attempt');
            return null;
        }

        // Verify token hasn't expired (24 hours)
        const tokenAge = Date.now() - result.Item.timestamp;
        if (tokenAge > 24 * 60 * 60 * 1000) {
            // Token expired, delete it
            await dynamoClient.send(new DeleteCommand({
                TableName: DYNAMODB_TABLE,
                Key: {
                    pk: `TOKEN#${token}`,
                    sk: 'TOKEN'
                }
            }));
            return null;
        }

        // Return the address from stored token data (not from decoded token)
        return result.Item.address.toLowerCase();
    } catch (e) {
        console.error('Token verification error:', e);
        return null;
    }
}

// Generate nonce from message+signature hash (one-time use)
function generateNonce(message, signature) {
    return crypto.createHash('sha256')
        .update(`${message}:${signature}`)
        .digest('hex');
}

// Check if nonce has been used before (prevents signature replay attacks)
async function isNonceUsed(nonce) {
    try {
        const result = await dynamoClient.send(new GetCommand({
            TableName: DYNAMODB_TABLE,
            Key: {
                pk: `NONCE#${nonce}`,
                sk: 'NONCE'
            }
        }));
        return !!result.Item;
    } catch (e) {
        console.error('Nonce check error:', e);
        throw new Error(`DynamoDB error checking nonce: ${e.message}. Check Lambda IAM permissions and table name: ${DYNAMODB_TABLE}`);
    }
}

// Mark nonce as used
async function markNonceUsed(nonce, address) {
    try {
        await dynamoClient.send(new PutCommand({
            TableName: DYNAMODB_TABLE,
            Item: {
                pk: `NONCE#${nonce}`,
                sk: 'NONCE',
                address: address.toLowerCase(),
                timestamp: Date.now(),
                ttl: Math.floor(Date.now() / 1000) + (7 * 24 * 60 * 60) // 7 days TTL for nonces
            }
        }));
    } catch (e) {
        console.error('Failed to mark nonce as used:', e);
        throw e;
    }
}

// Verify wallet signature with nonce check (one-time use)
async function verifySignature(address, message, signature) {
    try {
        // Generate nonce from message+signature
        const nonce = generateNonce(message, signature);
        
        // Check if this nonce has been used before
        let used;
        try {
            used = await isNonceUsed(nonce);
        } catch (e) {
            console.error('DynamoDB error checking nonce:', e);
            return { valid: false, reason: 'verification_error', error: `Database error: ${e.message}` };
        }
        
        if (used) {
            return { valid: false, reason: 'signature_already_used' };
        }
        
        // Recover address from signature
        const recoveredAddress = ethers.utils.verifyMessage(message, signature);
        // Check if recovered address matches provided address (case-insensitive)
        if (recoveredAddress.toLowerCase() !== address.toLowerCase()) {
            return { valid: false, reason: 'invalid_signature' };
        }
        // Check if address is in allowlist
        if (!ALLOWLIST.includes(address.toLowerCase())) {
            return { valid: false, reason: 'address_not_authorized' };
        }
        
        // Mark nonce as used (only after successful verification)
        try {
            await markNonceUsed(nonce, address);
        } catch (e) {
            // DynamoDB error storing nonce - log but don't fail auth (nonce is already checked)
            console.error('Warning: Failed to store nonce in DynamoDB:', e);
        }
        
        return { valid: true };
    } catch (e) {
        console.error('Signature verification error:', e);
        return { valid: false, reason: 'verification_error', error: e.message };
    }
}

// Get authorization from request
function getAuthFromRequest(event) {
    const authHeader = event.headers?.Authorization || event.headers?.authorization;
    if (!authHeader || !authHeader.startsWith('Bearer ')) {
        return null;
    }
    return authHeader.substring(7); // Remove 'Bearer ' prefix
}

// Check if request is authenticated
async function isAuthenticated(event) {
    const token = getAuthFromRequest(event);
    if (!token) return false;
    const address = await verifyToken(token);
    return address !== null;
}

export const handler = async (event) => {
    try {
        const httpMethod = event?.httpMethod || 'GET';
        
        const headers = {
            'Content-Type': 'application/json',
            'Access-Control-Allow-Origin': '*',
            'Access-Control-Allow-Methods': 'GET,POST,PUT,DELETE,OPTIONS',
            'Access-Control-Allow-Headers': 'Content-Type,Authorization'
        };

        // Handle CORS preflight
        if (httpMethod === 'OPTIONS') {
            return { statusCode: 200, headers, body: '' };
        }

    // Handle authentication endpoint
    if (event.path === '/auth' || event.path?.endsWith('/auth')) {
        if (httpMethod !== 'POST') {
            return { statusCode: 405, headers, body: JSON.stringify({ error: 'Method not allowed' }) };
        }

        try {
            const { address, message, signature } = JSON.parse(event.body || '{}');
            
            if (!address || !message || !signature) {
                return { statusCode: 400, headers, body: JSON.stringify({ error: 'Missing address, message, or signature' }) };
            }

            // Verify signature (includes nonce check)
            const verificationResult = await verifySignature(address, message, signature);
            if (!verificationResult.valid) {
                let errorMessage = 'Authentication failed';
                if (verificationResult.reason === 'signature_already_used') {
                    errorMessage = 'This signature has already been used. Please sign a new message.';
                } else if (verificationResult.reason === 'invalid_signature') {
                    errorMessage = 'Invalid signature. Please try connecting again.';
                } else if (verificationResult.reason === 'address_not_authorized') {
                    errorMessage = 'Address not authorized. Please contact administrator.';
                } else if (verificationResult.reason === 'verification_error') {
                    errorMessage = `Verification error: ${verificationResult.error}`;
                }
                return { statusCode: 401, headers, body: JSON.stringify({ error: errorMessage }) };
            }

            // Generate token (stored in DynamoDB)
            const token = await generateToken(address);

            return {
                statusCode: 200,
                headers,
                body: JSON.stringify({ token, address })
            };
        } catch (e) {
            console.error('Auth error:', e);
            const errorMessage = e.message || 'Authentication failed';
            if (errorMessage.includes('DynamoDB')) {
                return { 
                    statusCode: 500, 
                    headers, 
                    body: JSON.stringify({ 
                        error: 'Database error. Please check Lambda IAM permissions and DynamoDB configuration.'
                    }) 
                };
            }
            return { statusCode: 500, headers, body: JSON.stringify({ error: errorMessage }) };
        }
    }

    // All other endpoints require authentication
    const isAuth = await isAuthenticated(event);
    if (!isAuth) {
        return { statusCode: 401, headers, body: JSON.stringify({ error: 'Unauthorized. Please connect your wallet.' }) };
    }

    // API Gateway path handling
    const resourcePath = event.requestContext?.resourcePath;
    const originalPath = event.path || event.requestContext?.path || '/';
    
    let actualResourcePath = resourcePath;
    if (!actualResourcePath || actualResourcePath === '/') {
        let parsedPath = originalPath;
        if (parsedPath.startsWith('/dev/') || parsedPath.startsWith('/prod/') || parsedPath.startsWith('/staging/')) {
            parsedPath = parsedPath.replace(/^\/[^\/]+/, '');
        }
        actualResourcePath = parsedPath;
    }

    try {
        const campaigns = await loadCampaigns();
        
        if (httpMethod === 'GET') {
            const isCampaignsList = (actualResourcePath === '/campaigns' || actualResourcePath === '/campaigns/') && !event.pathParameters?.id;
            
            if (isCampaignsList) {
                return { statusCode: 200, headers, body: JSON.stringify({ campaigns }) };
            }
            
            let campaignId = event.pathParameters?.id;
            if (!campaignId) {
                const campaignsMatch = actualResourcePath.match(/\/campaigns\/([^\/]+)/);
                if (campaignsMatch) {
                    campaignId = campaignsMatch[1];
                }
            }
            if (campaignId) {
                const campaign = campaigns.find(c => c.id === campaignId);
                if (campaign) {
                    return { statusCode: 200, headers, body: JSON.stringify({ campaign }) };
                } else {
                    return { statusCode: 404, headers, body: JSON.stringify({ error: 'Campaign not found' }) };
                }
            }
        } else if (httpMethod === 'POST') {
            if (actualResourcePath === '/campaigns' || actualResourcePath === '/campaigns/') {
                const body = JSON.parse(event.body || '{}');
                const newCampaign = createCampaign(body, campaigns);
                await saveCampaigns(campaigns);
                return { statusCode: 201, headers, body: JSON.stringify({ campaign: newCampaign, message: 'Campaign created' }) };
            }
        } else if (httpMethod === 'PUT') {
            let campaignId = event.pathParameters?.id;
            if (!campaignId) {
                const campaignsMatch = actualResourcePath.match(/\/campaigns\/([^\/]+)/);
                if (campaignsMatch) {
                    campaignId = campaignsMatch[1];
                }
            }
            if (campaignId) {
                const body = JSON.parse(event.body || '{}');
                const updated = updateCampaign(campaignId, body, campaigns);
                if (updated) {
                    await saveCampaigns(campaigns);
                    return { statusCode: 200, headers, body: JSON.stringify({ campaign: updated, message: 'Campaign updated' }) };
                } else {
                    return { statusCode: 404, headers, body: JSON.stringify({ error: 'Campaign not found or cannot be edited' }) };
                }
            }
        } else if (httpMethod === 'DELETE') {
            let campaignId = event.pathParameters?.id;
            if (!campaignId) {
                const campaignsMatch = actualResourcePath.match(/\/campaigns\/([^\/]+)/);
                if (campaignsMatch) {
                    campaignId = campaignsMatch[1];
                }
            }
            if (campaignId) {
                const deleted = deleteCampaign(campaignId, campaigns);
                if (deleted) {
                    await saveCampaigns(campaigns);
                    return { statusCode: 200, headers, body: JSON.stringify({ message: 'Campaign deleted' }) };
                } else {
                    return { statusCode: 404, headers, body: JSON.stringify({ error: 'Campaign not found or cannot be deleted' }) };
                }
            }
        }

        return { statusCode: 405, headers, body: JSON.stringify({ error: 'Method not allowed' }) };
        
    } catch (e) {
        console.error("Error:", e);
        return { statusCode: 500, headers, body: JSON.stringify({ error: e.message || 'Internal server error' }) };
    }
    } catch (error) {
        // Top-level error handler - catch any unhandled errors
        console.error('Unhandled error in Lambda handler:', error);
        const headers = {
            'Content-Type': 'application/json',
            'Access-Control-Allow-Origin': '*',
            'Access-Control-Allow-Methods': 'GET,POST,PUT,DELETE,OPTIONS',
            'Access-Control-Allow-Headers': 'Content-Type,Authorization'
        };
        return {
            statusCode: 500,
            headers,
            body: JSON.stringify({
                error: 'Internal server error'
            })
        };
    }
};

async function loadCampaigns() {
    try {
        const command = new GetObjectCommand({
            Bucket: BUCKET,
            Key: KEY
        });
        const response = await s3Client.send(command);
        const content = await response.Body.transformToString();
        const config = parseToml(content);
        return config.campaigns || [];
    } catch (error) {
        if (error.name === 'NoSuchKey') {
            return [];
        }
        throw new Error(`Failed to load campaigns: ${error.message}`);
    }
}

async function saveCampaigns(campaigns) {
    const config = { campaigns };
    const content = stringifyToml(config);
    const command = new PutObjectCommand({
        Bucket: BUCKET,
        Key: KEY,
        Body: content,
        ContentType: 'text/plain'
    });
    await s3Client.send(command);
}

function validateCampaign(data) {
    const required = ['id', 'token_address', 'total_amount', 'start_date', 'end_date'];
    for (const field of required) {
        if (!(field in data)) {
            throw new Error(`Missing required field: ${field}`);
        }
    }
    
    const start = new Date(data.start_date);
    const end = new Date(data.end_date);
    if (isNaN(start) || isNaN(end) || end <= start) {
        throw new Error('end_date must be after start_date and valid dates');
    }
    
    if (parseFloat(data.total_amount) <= 0) {
        throw new Error('total_amount must be positive');
    }
    
    return {
        id: data.id,
        token_address: data.token_address,
        total_amount: parseFloat(data.total_amount),
        start_date: data.start_date,
        end_date: data.end_date,
        status: data.status || 'active'
    };
}

function canEditCampaign(campaign) {
    const today = new Date();
    today.setUTCHours(0, 0, 0, 0);
    const startDate = new Date(campaign.start_date);
    startDate.setUTCHours(0, 0, 0, 0);
    
    if (campaign.status === 'completed') return false;
    if (startDate <= today) return false;
    return true;
}

function canDeleteCampaign(campaign) {
    const today = new Date();
    today.setUTCHours(0, 0, 0, 0);
    const startDate = new Date(campaign.start_date);
    startDate.setUTCHours(0, 0, 0, 0);
    
    if (campaign.status === 'completed') return false;
    if (startDate <= today) return false;
    return true;
}

function createCampaign(data, campaigns) {
    if (campaigns.some(c => c.id === data.id)) {
        throw new Error(`Campaign ID '${data.id}' already exists`);
    }
    
    const campaign = validateCampaign(data);
    campaigns.push(campaign);
    return campaign;
}

function updateCampaign(campaignId, data, campaigns) {
    const index = campaigns.findIndex(c => c.id === campaignId);
    if (index === -1) {
        return null;
    }
    const existingCampaign = campaigns[index];
    
    const today = new Date();
    today.setUTCHours(0, 0, 0, 0);
    const startDate = new Date(existingCampaign.start_date);
    startDate.setUTCHours(0, 0, 0, 0);
    const isActive = startDate <= today && existingCampaign.status !== 'completed';
    
    if (isActive) {
        if (data.status && ['active', 'paused', 'completed'].includes(data.status)) {
            const hasOtherChanges = 
                data.token_address !== existingCampaign.token_address ||
                data.total_amount !== existingCampaign.total_amount ||
                data.start_date !== existingCampaign.start_date ||
                data.end_date !== existingCampaign.end_date;
            
            if (hasOtherChanges) {
                throw new Error('Cannot change campaign fields for active campaigns. Only status can be changed.');
            }
            
            campaigns[index].status = data.status;
            return campaigns[index];
        }
        throw new Error('Can only change status for active campaigns. Use pause/resume.');
    }
    
    if (!canEditCampaign(existingCampaign)) {
        throw new Error('Campaign cannot be edited (already started or completed)');
    }
    const updated = validateCampaign({ ...data, id: campaignId });
    campaigns[index] = updated;
    return updated;
}

function deleteCampaign(campaignId, campaigns) {
    const index = campaigns.findIndex(c => c.id === campaignId);
    if (index === -1) {
        return false;
    }
    const existingCampaign = campaigns[index];
    if (!canDeleteCampaign(existingCampaign)) {
        throw new Error('Campaign cannot be deleted (already started or completed). Use pause/complete instead.');
    }
    campaigns.splice(index, 1);
    return true;
}
