using System;
using System.Collections.Generic;
using System.IO;

namespace Hoi4ModdingSupporter.Util
{
    public class ProjectFile
    {
        public static void CreateProjeectFile(string projectName, string path)
        {
            FileStream stream = File.Create(path);
            string[] lines = { "projectName: " + projectName, "version: " + MainWindow.Version, "createTime: " + DateTime.Now.ToString() };

            StreamWriter writer = new StreamWriter(stream);

            foreach (string line in lines)
            {
                writer.WriteLine(line);
            }
            writer.Close();
            stream.Close();
        }

        public static List<string> ReadProjectFile(string path)
        {
            StreamReader reader = new StreamReader(path + @"\.project");
            List<string> list = new List<string>();

            string data;
            while ((data = reader.ReadLine()) != null)
                list.Add(data);
            reader.Close();
            reader.Dispose();

            return list;
        }

        public static void SaveProjectFile(string path)
        {
            try
            {
                StreamWriter writer = new StreamWriter(path + @"\.project");
                int row = File.ReadAllLines(path + @"\.project").Length;

                for (int i = 0; i < row; i++) {
                    if (i == (row - 1))
                    {
                        
                    }
                }
            }
            catch (IOException e)
            {
                Console.WriteLine(e.Message);
            }
        }

        public static void ParseProjectFile()
        {

        }
    }
}