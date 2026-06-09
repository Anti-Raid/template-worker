import { browser } from '$app/environment';
import { errorString, msyscall, Result } from './msyscall';
import type { MSyscallArgs, MSyscallError, MSyscallRet } from './msyscall/syscall';
import type { UserSession } from './msyscall/types/auth';
import type { PartialUser } from './msyscall/types/discord';

const defaultInstanceUrl = "http://localhost:60000"

export interface Session {
    session: UserSession;
    token: string;
    user: PartialUser;
}

const tryGetSession = (): Session | null => {
	if(!browser) return null
	try {
		let sess = localStorage.getItem('session')
		if(sess) return JSON.parse(sess)
		return null
	} catch {
		return null
	}
}

class Auth {
	session = $state(tryGetSession());
	instanceUrl = $state(browser ? localStorage.getItem('instance_url') ?? defaultInstanceUrl : defaultInstanceUrl);
	token = $derived(this.session?.token ?? null)

	save() {
		if (browser) {
			if(this.session) {
				localStorage.setItem('session', JSON.stringify(this.session));
			} else {
				localStorage.removeItem("session")
			}
			localStorage.setItem('instance_url', this.instanceUrl);
		}
	}

	async msyscall(call: MSyscallArgs): Promise<Result<MSyscallRet, MSyscallError>> {
		if(!this.instanceUrl) {
			let e: MSyscallError = {op: "Generic", message: "No instance URL set"}
			return new Result({ok: false, res: e as any})
		}
		return await msyscall(this.instanceUrl, this.token ?? undefined, call)
	}

	async getBotConfig() {
		let ret = (await this.msyscall({op: "Bot", req: {op: "GetBotConfig"}})).stringifyAndThrow(errorString)
		if(!(ret.op == "Bot" && ret.data.op == "BotConfig")) throw new Error("msyscall did not return a botconfig")
		return ret.data
	}

	async createLoginSession(code: string, redirectUri: string) {
		let ret = (await this.msyscall({op: "Auth", req: {op: "CreateLoginSession", code, redirect_uri: redirectUri}})).stringifyAndThrow(errorString)
		if(!(ret.op == "Auth" && ret.data.op == "CreatedSession")) throw new Error("msyscall did not return a session")
		return ret.data
	}
}

export const auth = new Auth();
