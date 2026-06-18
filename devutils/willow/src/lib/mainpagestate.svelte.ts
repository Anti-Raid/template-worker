import { browser } from "$app/environment";
import type { Page } from "./events.parse";
import type { RawKhronosValue } from "./msyscall/khronosvalue";
import type { BaseGuildUserInfo, DashboardGuild, DashboardGuildData } from "./msyscall/types/discord";

// note for future developers: | string -> error case
export interface State {
    refresh: boolean,
    showOnlyPresent: boolean,
    selectedGuild: DashboardGuild | null,
    fetchedUserGuilds: DashboardGuildData | string | null,
    baseGuildDatas: Record<string, BaseGuildUserInfo>,
    baseGuildDatasFetchErrors: Record<string, string>,
    dispatchEvent: { event: string, data: RawKhronosValue, fetched?: {data: RawKhronosValue} | string },
    settings: Record<string, Page>,
    settingsErr: [string, string][],
}

const defaultState: State = {
    refresh: false,
    showOnlyPresent: false,
    selectedGuild: null,
    fetchedUserGuilds: null,
    baseGuildDatas: {},
    baseGuildDatasFetchErrors: {},
    dispatchEvent: { event: "", data: {Nil: null}},
    settings: {},
    settingsErr: []
    
}
export const stateKey = "mainpagestate.willowv1"

const getState = (): State => {
    if(!browser) return defaultState
    try {
        let sessData = localStorage.getItem(stateKey)
        if(sessData) {
            let state: State = JSON.parse(sessData)
            // Merge with default state to handle missing fields in old saves
            return { ...defaultState, ...state }
        }
        return defaultState
    } catch {
        return defaultState
    }
}

class MainPageState {
    state = $state(getState())
    roleChoices = $derived.by<{label: string, value: string}[]>(() => {
        if (!this.state.selectedGuild) return []
        let gbi = mps.state.baseGuildDatas[this.state.selectedGuild.id]
        if(!gbi) return []
        return gbi.roles.map(x => {return {label: x.name, value: x.id}})
    })
    channelChoices = $derived.by<{label: string, value: string}[]>(() => {
        if (!this.state.selectedGuild) return []
        let gbi = mps.state.baseGuildDatas[this.state.selectedGuild.id]
        if(!gbi) return []
        // type 4 = GUILD_CATEGORY
        return gbi.channels.filter(x => x.channel.type != 4).map(x => {return {label: x.channel.name, value: x.channel.id}})
    })

    save() {
        localStorage.setItem(stateKey, JSON.stringify(this.state));
    }

    /**
     * Returns the current state as a JSON string
     */
    export(): string {
        return JSON.stringify(this.state);
    }

    /**
     * Imports state from a JSON string
     */
    import(json: string) {
        try {
            const parsed = JSON.parse(json);
            // Basic validation: ensure it's an object
            if (parsed && typeof parsed === 'object') {
                this.state = { ...this.state, ...parsed }
            }
        } catch (e) {
            throw new Error(`Failed to import state: ${e}`);
        }
    }
}

export const mps = new MainPageState();
