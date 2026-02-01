
import { Moon, Sun, Monitor, Palette } from "lucide-react"
import { useTheme, type Theme, type ColorScheme } from "@/hooks/use-theme"
import { useState, useRef, useEffect } from "react"

export function ThemeSelector() {
  const { theme, setTheme, colorScheme, setColorScheme } = useTheme()
  const [isOpen, setIsOpen] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }
    document.addEventListener("mousedown", handleClickOutside)
    return () => document.removeEventListener("mousedown", handleClickOutside)
  }, [])

  const themes: { value: Theme; icon: React.ReactNode; label: string }[] = [
    { value: "light", icon: <Sun size={16} />, label: "Light" },
    { value: "dark", icon: <Moon size={16} />, label: "Dark" },
    { value: "system", icon: <Monitor size={16} />, label: "System" },
  ]

  const schemes: { value: ColorScheme; label: string; color: string }[] = [
    { value: "default", label: "Default", color: "bg-zinc-500" },
    { value: "ocean", label: "Ocean", color: "bg-blue-500" },
    { value: "forest", label: "Forest", color: "bg-green-500" },
    { value: "sunset", label: "Sunset", color: "bg-orange-500" },
  ]

  return (
    <div className="relative" ref={menuRef}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="p-2 rounded-md hover:bg-secondary text-secondary-foreground transition-colors flex items-center gap-2"
        title="Theme Settings"
      >
        <Palette size={20} />
      </button>

      {isOpen && (
        <div className="absolute right-0 mt-2 w-64 rounded-md border border-border bg-popover p-4 shadow-lg z-50 text-popover-foreground">
          <div className="space-y-4">
            <div>
              <h3 className="text-sm font-medium mb-2">Theme Mode</h3>
              <div className="flex bg-muted rounded-lg p-1">
                {themes.map((t) => (
                  <button
                    key={t.value}
                    onClick={() => setTheme(t.value)}
                    className={`flex-1 flex items-center justify-center p-1.5 rounded-md text-sm transition-all ${
                      theme === t.value
                        ? "bg-background shadow-sm text-foreground"
                        : "text-muted-foreground hover:text-foreground"
                    }`}
                    title={t.label}
                  >
                    {t.icon}
                  </button>
                ))}
              </div>
            </div>

            <div>
              <h3 className="text-sm font-medium mb-2">Color Scheme</h3>
              <div className="grid grid-cols-4 gap-2">
                {schemes.map((s) => (
                  <button
                    key={s.value}
                    onClick={() => setColorScheme(s.value)}
                    className={`group relative flex flex-col items-center gap-1 p-2 rounded-md border-2 transition-all ${
                      colorScheme === s.value
                        ? "border-primary bg-muted"
                        : "border-transparent hover:bg-muted"
                    }`}
                    title={s.label}
                  >
                    <div className={`w-6 h-6 rounded-full ${s.color}`} />
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
