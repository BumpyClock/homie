
import { createContext, useContext, useEffect, useState } from "react"

export type Theme = "dark" | "light" | "system"

export const COLOR_SCHEMES = [
  "default",
  "monokai",
  "one-dark",
  "flexoki",
  "dracula",
  "catppuccin",
] as const

export type ColorScheme = (typeof COLOR_SCHEMES)[number]

interface ThemeProviderProps {
  children: React.ReactNode
  defaultTheme?: Theme
  defaultScheme?: ColorScheme
  storageKey?: string
}

interface ThemeProviderState {
  theme: Theme
  setTheme: (theme: Theme) => void
  colorScheme: ColorScheme
  setColorScheme: (scheme: ColorScheme) => void
  resolvedTheme: "dark" | "light" // The actual active theme (resolving system)
}

const initialState: ThemeProviderState = {
  theme: "system",
  setTheme: () => null,
  colorScheme: "default",
  setColorScheme: () => null,
  resolvedTheme: "dark",
}

const ThemeProviderContext = createContext<ThemeProviderState>(initialState)

export function ThemeProvider({
  children,
  defaultTheme = "system",
  defaultScheme = "default",
  storageKey = "vite-ui-theme",
}: ThemeProviderProps) {
  const [theme, setThemeState] = useState<Theme>(
    () => (localStorage.getItem(storageKey + "-mode") as Theme) || defaultTheme
  )
  
  const [colorScheme, setColorSchemeState] = useState<ColorScheme>(() => {
    const stored = localStorage.getItem(storageKey + "-scheme")
    if (stored && (COLOR_SCHEMES as readonly string[]).includes(stored)) {
      return stored as ColorScheme
    }
    return defaultScheme
  })

  const [resolvedTheme, setResolvedTheme] = useState<"dark" | "light">("dark");

  useEffect(() => {
    const root = window.document.documentElement
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)")

    const applyTheme = () => {
        root.classList.remove("light", "dark")
        let effectiveTheme: "dark" | "light"

        if (theme === "system") {
             effectiveTheme = mediaQuery.matches ? "dark" : "light"
        } else {
             effectiveTheme = theme
        }
        
        root.classList.add(effectiveTheme)
        setResolvedTheme(effectiveTheme)
    }

    applyTheme()

    const handleChange = () => {
        if (theme === "system") {
            applyTheme()
        }
    }
    
    mediaQuery.addEventListener('change', handleChange)
    return () => mediaQuery.removeEventListener('change', handleChange)
  }, [theme])
  
  useEffect(() => {
      const root = window.document.documentElement;
      root.setAttribute('data-color-scheme', colorScheme);
  }, [colorScheme])

  const setTheme = (theme: Theme) => {
    localStorage.setItem(storageKey + "-mode", theme)
    setThemeState(theme)
  }
  
  const setColorScheme = (scheme: ColorScheme) => {
      localStorage.setItem(storageKey + "-scheme", scheme)
      setColorSchemeState(scheme)
  }

  const value = {
    theme,
    setTheme,
    colorScheme,
    setColorScheme,
    resolvedTheme
  }

  return (
    <ThemeProviderContext.Provider value={value}>
      {children}
    </ThemeProviderContext.Provider>
  )
}

// eslint-disable-next-line react-refresh/only-export-components
export const useTheme = () => {
  const context = useContext(ThemeProviderContext)

  if (context === undefined)
    throw new Error("useTheme must be used within a ThemeProvider")

  return context
}
