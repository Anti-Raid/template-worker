import { auth } from '$lib/auth.svelte';

export const doLogin = async (onStateChange?: (state: string) => void): Promise<void> => {
    onStateChange?.("GetBotConfig");
    let bcfg = await auth.getBotConfig();
    
    onStateChange?.("WaitForUserOauth");
    let loginUrl = `https://discord.com/api/oauth2/authorize?client_id=${bcfg.client_id}&scope=identify%20guilds&redirect_uri=${window.location.origin}/authorize&response_type=code`;
    const popup = window.open(loginUrl, "Oauth2 Login", "popup");
    if (!popup) {
        throw new Error("Popup blocked! Please allow popups for this site.");
    }

    const authData: string = await new Promise((resolve, reject) => {   
        const checkPopup = setInterval(() => {
            if (popup.closed) {
                clearInterval(checkPopup);
                window.removeEventListener("message", messageListener);
                reject(new Error("Login popup closed"));
            }
        }, 500);

        const messageListener = (event: MessageEvent<any>) => {
            if (event.origin !== window.location.origin) return;

            if (event.data && event.data.op === "GotCode") {
                clearInterval(checkPopup);
                window.removeEventListener("message", messageListener);
                popup.close();
                resolve(event.data.code);
            }
        };

        window.addEventListener("message", messageListener);
    });

    if(authData == null) {
        throw new Error("User cancelled login request?");
    }

    onStateChange?.(`GotCode(${authData})`);

    let sess = await auth.createLoginSession(authData, `${window.location.origin}/authorize`);
    if(!sess.user) throw new Error("CreateLoginSession did not return a user as it is required to!");
    
    auth.session = {session: sess.session, token: sess.token, user: sess.user};
    auth.save();
};
