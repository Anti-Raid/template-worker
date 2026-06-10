import { browser } from "$app/environment";
import type { DashboardGuild, DashboardGuildData } from "./msyscall/types/discord";

export interface State {
    refresh: boolean,
    showOnlyPresent: boolean,
    selectedGuild: DashboardGuild | null,
    fetchedUserGuilds: DashboardGuildData | string | null
}

const defaultState: State = {
    refresh: false,
    showOnlyPresent: false,
    selectedGuild: null,
    fetchedUserGuilds: null
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
