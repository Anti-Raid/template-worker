import { browser } from '$app/environment';
import { errorString, msyscall, Result } from './msyscall';
import { dexpand, expand, type RawKhronosValue } from './msyscall/khronosvalue';
import type { MSyscallArgs, MSyscallError, MSyscallRet } from './msyscall/syscall';
import type { UserSession } from './msyscall/types/auth';
import type { Id } from './msyscall/types/common';
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
		if(sess) {
			let session: Session = JSON.parse(sess)
			return session
		}
		return null
	} catch {
		return null
	}
}

class Auth {
	instanceUrl = $state(browser ? localStorage.getItem('instance_url') ?? defaultInstanceUrl : defaultInstanceUrl);
	session = $state(tryGetSession());
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

	async checkAuth(): Promise<boolean> {
		// technically a hack but until msyscall gets an API for this
		if(!this.token) return false
		let ret = (await this.msyscall({op: "Auth", req: {op: "GetUserSessions"}}))
		if(ret.ok) return true // we successfully performed an authorized op
		let err = ret.unwrapErr()
		console.log(err, err.op)
		if (err.op == "Unauthorized") return false // not unauthorized
		if (err.op == "Generic" && err.message.includes("Failed to fetch")) return false
		return true
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

	async getUserGuilds(refresh: boolean) {
		let ret = (await this.msyscall({op: "Discord", req: {op: "GetUserGuilds", refresh }})).stringifyAndThrow(errorString)
		if(!(ret.op == "Discord" && ret.data.op == "UserGuilds")) throw new Error("msyscall did not return user guilds")
		return ret.data
	}

	async dispatchEvent(id: Id, name: string, data: RawKhronosValue, compressed: boolean = true) {
		let ret = (await this.msyscall({op: "Bot", req: compressed ? {op: "DispatchCEvent", id, name, data: dexpand(data)} : {op: "DispatchEvent", id, name, data }})).stringifyAndThrow(errorString)
		if(!(ret.op == "Bot" && (ret.data.op == "KhronosValue" || ret.data.op == "CKhronosValue"))) throw new Error("msyscall did not return a khronos value")
		return ret.data.op == "CKhronosValue" ? expand(ret.data.data) : ret.data.data
	}

	async getGuildInfo(guild_id: string) {
		let ret = (await this.msyscall({op: "Discord", req: {op: "GetGuildInfo", guild_id }})).stringifyAndThrow(errorString)
		if(!(ret.op == "Discord" && ret.data.op == "GuildInfo")) throw new Error("msyscall did not return guild info")
		return ret.data.data
	}
}

export const auth = new Auth();
