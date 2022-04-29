using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

namespace Hoi4ModdingSupporter.Util
{
    public class Setting
    {
        [DllImport("kernel32.dll")]
        private static extern uint GetPrivateProfileString(string lpAppName, string lpKeyName, string lpDefault, StringBuilder lpReturnedString, uint nSize, string lpFileName);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool WritePrivateProfileString(string lpAppName, string lpKeyName, string lpString, string lpFileName);

        private readonly string ConfigFilePath;
        public bool NotificateUpdate { get; set; }
        public string GamePath { get; set; }

        public Setting(string configFilePath)
        {
            this.ConfigFilePath = configFilePath;
        }

        public void InitConfig()
        {
            File.Create(ConfigFilePath);
            string[] contents = { "[Data]", "NotificateUpdate=true", "GamePath=" };
            File.WriteAllLines(ConfigFilePath, contents);
            LoadConfig();
        }

        public void LoadConfig()
        {
            try
            {
                StringBuilder sb = new StringBuilder();
                GetPrivateProfileString("Data", "NotificateUpdate", "none", sb, Convert.ToUInt32(sb.Capacity), ConfigFilePath);
                StreamReader streamReader = new StreamReader(File.Open(ConfigFilePath, FileMode.Open));
                
            }
            catch (FileNotFoundException)
            {
                InitConfig();
            }
        }

        public void SaveConfig()
        {

        }
    }
}