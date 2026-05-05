import { defineStore } from 'pinia';
import { setNativeTheme } from '../runtime/host';

interface ThemeState {
  userPreference: 'light' | 'dark' | 'system';
  currentSystemTheme: 'light' | 'dark';
}

const STORE_KEY = 'theme_preference';

export const useThemeStore = defineStore('theme', {
  state: (): ThemeState => ({
    userPreference: 'system', // Default to 'system'
    currentSystemTheme: 'light', // Will be updated
  }),
  actions: {
    async initTheme() {
      // Load user preference from localStorage
      const storedPreference = localStorage.getItem(STORE_KEY);
      if (storedPreference && ['light', 'dark', 'system'].includes(storedPreference)) {
        this.userPreference = storedPreference as ThemeState['userPreference'];
      } else {
        this.userPreference = 'system'; // Default if nothing stored or invalid
      }
      
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      this.currentSystemTheme = mediaQuery.matches ? 'dark' : 'light';

      mediaQuery.addEventListener('change', (e) => {
        this.currentSystemTheme = e.matches ? 'dark' : 'light';
        this._applyTheme();
      });

      this._applyTheme();
    },

    setUserPreference(preference: 'light' | 'dark' | 'system') {
      if (this.userPreference === preference) return;
      this.userPreference = preference;
      localStorage.setItem(STORE_KEY, preference);
      this._applyTheme();
    },

    toggleTheme() {
      const nextTheme = this.getEffectiveTheme() === 'light' ? 'dark' : 'light';
      this.setUserPreference(nextTheme);
    },

    async _applyTheme() {
      let themeToApply: 'light' | 'dark';
      if (this.userPreference === 'system') {
        themeToApply = this.currentSystemTheme;
      } else {
        themeToApply = this.userPreference;
      }
      document.documentElement.setAttribute('data-theme', themeToApply);

      await setNativeTheme(themeToApply);
    },

    // Getter to easily access the currently active theme in components
    getEffectiveTheme(): 'light' | 'dark' {
      if (this.userPreference === 'system') {
        return this.currentSystemTheme;
      }
      return this.userPreference;
    }
  },
}); 
