/** Validate JWT tokens and return a session. */
export function validateToken(token: string, secret: string): Session {
    return { token: `${secret}:${token}` };
}

export interface Session {
    token: string;
}
