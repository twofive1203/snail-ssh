/** 把设置里的参数字符串拆成数组。认引号，不交给 cmd.exe。 */
export function splitShellArgs(input: string): string[] {
  const args: string[] = []
  let current = ''
  let quote: '"' | "'" | null = null
  for (const ch of input) {
    if (quote) {
      if (ch === quote) quote = null
      else current += ch
      continue
    }
    if (ch === '"' || ch === "'") {
      quote = ch
      continue
    }
    if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
      if (current) {
        args.push(current)
        current = ''
      }
      continue
    }
    current += ch
  }
  if (current) args.push(current)
  return args
}
