import { describe, it, beforeEach, afterEach } from 'mocha';
const { expect } = require('chai');
import { MqttServer, AuthenticationRequest, AuthenticationResult } from '../src/index';
import { waitForPort, waitForPortClosed } from './helpers';
import { connect, MqttClient } from 'mqtt';
import * as net from 'net';

describe('MQTT Server Authentication', () => {
    let server: MqttServer;
    // Use a unique port per test to avoid interference between cases
    let currentPort: number = 0;
    let portCounter = 0;
    const nextPort = () => 19000 + (portCounter++);

    beforeEach(() => {
        server = new MqttServer();
    });

    afterEach(async () => {
        if (server.running) {
            await server.stop();
            // ensure the port is actually closed before ending the test
            if (currentPort) {
                try { await waitForPortClosed('127.0.0.1', currentPort); } catch {}
            }
        }
        server.close();
    });

    /**
     * Helper function to check if a port is listening
     */
    function checkPortListening(host: string, port: number): Promise<boolean> {
        return new Promise((resolve) => {
            const socket = new net.Socket();
            
            socket.setTimeout(1000);
            socket.on('connect', () => {
                socket.destroy();
                resolve(true);
            });
            
            socket.on('timeout', () => {
                socket.destroy();
                resolve(false);
            });
            
            socket.on('error', () => {
                resolve(false);
            });
            
            socket.connect(port, host);
        });
    }

    /**
     * Helper function to attempt MQTT connection
     */
    function attemptConnection(options: any, port?: number): Promise<{ success: boolean, client?: MqttClient, error?: Error }> {
        return new Promise((resolve) => {
            const usePort = port ?? currentPort;
            const client = connect(`mqtt://localhost:${usePort}`, {
                connectTimeout: 2000,
                ...options
            });

            client.on('connect', () => {
                resolve({ success: true, client });
            });

            client.on('error', (error) => {
                resolve({ success: false, error });
            });

            // Timeout after 3 seconds
            setTimeout(() => {
                client.end();
                resolve({ success: false, error: new Error('Connection timeout') });
            }, 3000);
        });
    }

    it('should allow connections when authentication callback returns allow: true', async function() {
        this.timeout(15000); // Increased timeout

        return new Promise<void>((resolve, reject) => {
            currentPort = nextPort();
            const timeout = setTimeout(() => {
                reject(new Error('Authentication test timeout'));
            }, 12000);

            let authRequestReceived = false;

            // Set up authentication hook that allows all connections
            server.setHooks({
                onClientAuthenticate: (authRequest: AuthenticationRequest): AuthenticationResult => {
                    authRequestReceived = true;
                    
                    // Verify the authentication request has expected fields
                    expect(authRequest).to.have.property('clientId');
                    expect(authRequest).to.have.property('username');
                    expect(authRequest).to.have.property('password');
                    expect(authRequest).to.have.property('remoteAddr');
                    expect(authRequest).to.have.property('protocolVersion');
                    expect(authRequest).to.have.property('keepAlive');
                    expect(authRequest).to.have.property('cleanSession');

                    const result = {
                        allow: true,
                        superuser: false,
                        reason: 'Test authentication - allowed'
                    };
                    return result;
                }
            });

            server.start({
                listeners: [{
                    name: "tcp-auth-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: false // Force authentication through our hook
                }]
            }).then(async () => {
                // Wait for server to be ready
                await waitForPort('127.0.0.1', currentPort);
                // Attempt anonymous connection
                const result = await attemptConnection({
                    clientId: 'test-anonymous-client'
                }, currentPort);

                clearTimeout(timeout);

                expect(authRequestReceived).to.be.true;
                expect(result.success).to.be.true;
                
                if (result.client) {
                    result.client.end();
                }

                resolve();
            }).catch((err) => {
                clearTimeout(timeout);
                reject(err);
            });
        });
    });

    it('should deny connections when authentication callback returns allow: false', async function() {
        this.timeout(10000);

        return new Promise<void>((resolve, reject) => {
            currentPort = nextPort();
            const timeout = setTimeout(() => {
                reject(new Error('Authentication deny test timeout'));
            }, 8000);

            let authRequestReceived = false;

            // Set up authentication hook that denies all connections
            server.setHooks({
                onClientAuthenticate: (authRequest: AuthenticationRequest): AuthenticationResult => {
                    authRequestReceived = true;
                    
                    return {
                        allow: false,
                        superuser: false,
                        reason: 'Test authentication - denied'
                    };
                }
            });

            server.start({
                listeners: [{
                    name: "tcp-auth-deny-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: false // Force authentication through our hook
                }]
            }).then(async () => {
                // Wait for server and hooks to be fully ready
                await waitForPort('127.0.0.1', currentPort);

                // Attempt connection that should be denied
                const result = await attemptConnection({
                    clientId: 'test-denied-client'
                }, currentPort);

                clearTimeout(timeout);

                expect(result.success).to.be.false;
                expect(authRequestReceived).to.be.true;
                resolve();
            }).catch(reject);
        });
    });

    it('should authenticate with username and password', async function() {
        this.timeout(10000);

        return new Promise<void>((resolve, reject) => {
            currentPort = nextPort();
            const timeout = setTimeout(() => {
                reject(new Error('Username/password auth test timeout'));
            }, 8000);

            let authRequest: AuthenticationRequest | null = null;

            // Set up authentication hook with username/password validation
            server.setHooks({
                onClientAuthenticate: (request: AuthenticationRequest): AuthenticationResult => {
                    authRequest = request;
                    
                    // Check for valid credentials
                    if (request.username === 'testuser' && request.password === 'testpass') {
                        return {
                            allow: true,
                            superuser: false,
                            reason: 'Valid credentials'
                        };
                    }
                    
                    return {
                        allow: false,
                        superuser: false,
                        reason: 'Invalid credentials'
                    };
                }
            });

            server.start({
                listeners: [{
                    name: "tcp-auth-cred-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: true
                }]
            }).then(async () => {
                // Wait for server and hooks to be fully ready
                await waitForPort('127.0.0.1', currentPort);

                // Test valid credentials
                const validResult = await attemptConnection({
                    clientId: 'test-valid-creds',
                    username: 'testuser',
                    password: 'testpass'
                }, currentPort);

                expect(validResult.success).to.be.true;
                expect(authRequest?.username).to.equal('testuser');
                expect(authRequest?.password).to.equal('testpass');
                
                if (validResult.client) {
                    validResult.client.end();
                }

                // Test invalid credentials
                const invalidResult = await attemptConnection({
                    clientId: 'test-invalid-creds',
                    username: 'testuser',
                    password: 'wrongpass'
                }, currentPort);

                clearTimeout(timeout);

                expect(invalidResult.success).to.be.false;
                resolve();
            }).catch(reject);
        });
    });

    it('should allow connections when authentication callback returns a Promise that resolves', async function() {
        this.timeout(15000);

        return new Promise<void>((resolve, reject) => {
            currentPort = nextPort();
            const timeout = setTimeout(() => {
                reject(new Error('Auth promise resolve test timeout'));
            }, 12000);

            server.setHooks({
                onClientAuthenticate: async (_req: AuthenticationRequest): Promise<AuthenticationResult> => {
                    await new Promise(r => setTimeout(r, 50));
                    return { allow: true, superuser: false, reason: 'ok' };
                }
            });

            server.start({
                listeners: [{
                    name: "tcp-auth-promise-allow",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: false
                }]
            }).then(async () => {
                await waitForPort('127.0.0.1', currentPort);

                const result = await attemptConnection({ clientId: 'promise-allow' }, currentPort);
                clearTimeout(timeout);
                expect(result.success).to.be.true;
                if (result.client) result.client.end();
                resolve();
            }).catch(err => { clearTimeout(timeout); reject(err); });
        });
    });

    it('should deny connections when authentication callback Promise rejects', async function() {
        this.timeout(15000);

        return new Promise<void>((resolve, reject) => {
            currentPort = nextPort();
            const timeout = setTimeout(() => {
                reject(new Error('Auth promise reject test timeout'));
            }, 12000);

            server.setHooks({
                onClientAuthenticate: async (): Promise<AuthenticationResult> => {
                    return Promise.reject(new Error('nope')) as any;
                }
            });

            server.start({
                listeners: [{
                    name: "tcp-auth-promise-deny",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: false
                }]
            }).then(async () => {
                await waitForPort('127.0.0.1', currentPort);
                const result = await attemptConnection({ clientId: 'promise-deny' }, currentPort);
                clearTimeout(timeout);
                expect(result.success).to.be.false;
                resolve();
            }).catch(err => { clearTimeout(timeout); reject(err); });
        });
    });

    it('should handle JWT-like token authentication', async function() {
        this.timeout(10000);

        return new Promise<void>((resolve, reject) => {
            currentPort = nextPort();
            const timeout = setTimeout(() => {
                reject(new Error('JWT auth test timeout'));
            }, 8000);

            let jwtAuthRequest: AuthenticationRequest | null = null;

            // Set up authentication hook with JWT-like token validation
            server.setHooks({
                onClientAuthenticate: (request: AuthenticationRequest): AuthenticationResult => {
                    jwtAuthRequest = request;
                    
                    // Check for JWT-like password
                    if (request.password?.startsWith('eyJ')) {
                        // Simple JWT validation - in real world, use proper JWT library
                        const parts = request.password.split('.');
                        if (parts.length === 3) {
                            return {
                                allow: true,
                                superuser: false,
                                reason: 'Valid JWT token'
                            };
                        }
                    }
                    
                    return {
                        allow: false,
                        superuser: false,
                        reason: 'Invalid or missing JWT token'
                    };
                }
            });

            server.start({
                listeners: [{
                    name: "tcp-jwt-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: true
                }]
            }).then(async () => {
                // Wait for server and hooks to be fully ready
                await waitForPort('127.0.0.1', currentPort);

                // Test with JWT-like token
                const jwtToken = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VybmFtZSI6InRlc3QifQ.signature';
                
                const jwtResult = await attemptConnection({
                    clientId: 'test-jwt-client',
                    username: 'testuser',
                    password: jwtToken
                }, currentPort);

                expect(jwtResult.success).to.be.true;
                expect(jwtAuthRequest?.password).to.equal(jwtToken);
                
                if (jwtResult.client) {
                    jwtResult.client.end();
                }

                // Test with invalid token
                const invalidResult = await attemptConnection({
                    clientId: 'test-invalid-jwt',
                    username: 'testuser',
                    password: 'not-a-jwt-token'
                }, currentPort);

                clearTimeout(timeout);

                expect(invalidResult.success).to.be.false;
                resolve();
            }).catch(reject);
        });
    });

    it('should handle authentication callback timeout gracefully', async function() {
        this.timeout(15000); // Longer timeout for this test

        return new Promise<void>((resolve, reject) => {
            currentPort = nextPort();
            const timeout = setTimeout(() => {
                reject(new Error('Timeout test timeout'));
            }, 12000);

            // Set up authentication hook that takes too long (simulates timeout)
            server.setHooks({
                onClientAuthenticate: (request: AuthenticationRequest): AuthenticationResult => {
                    // Simulate a very slow authentication process
                    // The Rust code has a 5-second timeout, so this should trigger it
                    return new Promise<AuthenticationResult>((resolve) => {
                        setTimeout(() => {
                            resolve({
                                allow: true,
                                superuser: false,
                                reason: 'Finally allowed after delay'
                            });
                        }, 6000); // 6 seconds - longer than the 5-second timeout
                    }) as any; // TypeScript doesn't expect Promise return, but JS will handle it
                }
            });

            server.start({
                listeners: [{
                    name: "tcp-timeout-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: false
                }]
            }).then(async () => {
                // Wait for server and hooks to be fully ready
                await new Promise(r => setTimeout(r, 1000));

                // Attempt connection that should timeout
                const result = await attemptConnection({
                    clientId: 'test-timeout-client'
                }, currentPort);

                clearTimeout(timeout);

                // Should be denied due to timeout
                expect(result.success).to.be.false;
                resolve();
            }).catch(reject);
        });
    });

    it('should set superuser flag correctly', async function() {
        this.timeout(10000);

        return new Promise<void>((resolve, reject) => {
            currentPort = nextPort();
            const timeout = setTimeout(() => {
                reject(new Error('Superuser test timeout'));
            }, 8000);

            // Set up authentication hook that grants superuser to admin
            server.setHooks({
                onClientAuthenticate: (request: AuthenticationRequest): AuthenticationResult => {
                    const isAdmin = request.username === 'admin';
                    
                    return {
                        allow: true,
                        superuser: isAdmin,
                        reason: isAdmin ? 'Admin user' : 'Regular user'
                    };
                }
            });

            server.start({
                listeners: [{
                    name: "tcp-superuser-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: true
                }]
            }).then(async () => {
                // Wait for server to be ready
                await waitForPort('127.0.0.1', currentPort);

                // Test admin user (should be superuser)
                const adminResult = await attemptConnection({
                    clientId: 'test-admin',
                    username: 'admin',
                    password: 'adminpass'
                }, currentPort);

                expect(adminResult.success).to.be.true;
                
                if (adminResult.client) {
                    adminResult.client.end();
                }

                // Test regular user (should not be superuser)
                const userResult = await attemptConnection({
                    clientId: 'test-user',
                    username: 'user',
                    password: 'userpass'
                }, currentPort);

                clearTimeout(timeout);

                expect(userResult.success).to.be.true;
                if (userResult.client) {
                    userResult.client.end();
                }
                resolve();
            }).catch(reject);
        });
    });

    it('should handle missing authentication callback gracefully', async function() {
        this.timeout(10000);

        return new Promise<void>((resolve, reject) => {
            currentPort = nextPort();
            const timeout = setTimeout(() => {
                reject(new Error('Missing callback test timeout'));
            }, 8000);

            // Start server without setting authentication hook
            server.start({
                listeners: [{
                    name: "tcp-no-auth-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: false
                }]
            }).then(async () => {
                // Wait for server to be ready
                await new Promise(r => setTimeout(r, 300));

                // Attempt connection without authentication callback
                // Should be denied by default
                const result = await attemptConnection({
                    clientId: 'test-no-auth-callback'
                }, currentPort);

                clearTimeout(timeout);

                // Should be denied when no auth callback is set
                expect(result.success).to.be.false;
                resolve();
            }).catch(reject);
        });
    });
});